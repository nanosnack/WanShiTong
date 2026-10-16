use crate::commands::start_ingest;
use crate::context::TaskId;
use crate::queries::list_data_sources;
use crate::report_error;
use crate::{AppContext, AppError};
use notify::event::{AccessKind, AccessMode, ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};
use tracing::{error, info};
use uuid::Uuid;
use wanshitong_core::domain::{DataSourceConfig, DataSourceUuid};

/// How long a file must go unwritten, though it's still open for writing,
/// before its data source is ingested anyway, e.g. a download that stalled.
const STALLED_WRITE: Duration = Duration::from_secs(10 * 60);

/// Starts watching the data sources' files, to ingest a data source once its
/// files have changed and then gone unchanged for the configured quiet period,
/// unless a file in it is still being written, e.g. copied in: then once that's
/// done. The data sources there are when it returns are being watched, and those
/// created or restored later are watched from then on.
///
/// Changes made while nothing watches, e.g. while the service is stopped, are
/// only seen by the next ingest.
pub async fn watch_data_sources(ctx: &AppContext) -> Result<(), AppError> {
    let (events_tx, events) = mpsc::unbounded_channel();
    let watcher = notify::recommended_watcher(move |event| {
        // Only fails once the watch task, and so the receiver, is gone.
        let _ = events_tx.send(event);
    })?;
    let mut watch = Watch {
        ctx: ctx.clone(),
        watcher: Arc::new(Mutex::new(watcher)),
        roots: HashSet::new(),
        sources: Vec::new(),
        quiet_period: Duration::from_secs(ctx.config().watch_quiet_period_secs),
        due: HashMap::new(),
        writing: HashMap::new(),
    };
    watch.sync().await;

    let task_id = TaskId(Uuid::new_v4());
    let mut tasks = ctx.tasks.lock().await;
    let task = tokio::spawn(watch.run(events));
    tasks.insert(task_id, task);
    Ok(())
}

struct Watch {
    ctx: AppContext,
    /// Shared so that watching, which walks a whole directory tree, can run on
    /// a blocking thread.
    watcher: Arc<Mutex<RecommendedWatcher>>,
    /// Directories being watched.
    roots: HashSet<PathBuf>,
    /// Each data source and its root, which may not be watched yet.
    sources: Vec<(DataSourceUuid, PathBuf)>,
    quiet_period: Duration,
    /// When to ingest each data source whose files changed, unless they
    /// change again first.
    due: HashMap<DataSourceUuid, Instant>,
    /// Files written to and not closed since, and when each was last written.
    writing: HashMap<PathBuf, Instant>,
}

impl Watch {
    async fn run(mut self, mut events: mpsc::UnboundedReceiver<notify::Result<Event>>) {
        let data_sources_changed = self.ctx.data_sources_changed.clone();
        loop {
            let next_due = self.due.values().min().copied();
            tokio::select! {
                () = data_sources_changed.notified() => self.sync().await,
                Some(event) = events.recv() => self.event(event),
                // The sleep is created even when disabled, so it needs some
                // instant.
                () = sleep_until(next_due.unwrap_or_else(Instant::now)), if next_due.is_some() => {
                    self.ingest_due().await;
                }
            }
        }
    }

    /// Watches the data sources there are now, and stops watching the rest.
    /// A root that couldn't be watched, e.g. because it doesn't exist, is
    /// tried again on the next sync.
    async fn sync(&mut self) {
        let sources = match list_data_sources(&self.ctx).await {
            Ok(sources) => sources,
            Err(e) => {
                report_error!(e, "Couldn't list the data sources to watch");
                return;
            }
        };
        self.sources = sources
            .into_iter()
            .map(|source| match source.config {
                DataSourceConfig::FileSystem(fs) => (source.uuid, fs.root_path),
            })
            .collect();
        self.due
            .retain(|uuid, _| self.sources.iter().any(|(source, _)| source == uuid));

        let wanted: HashSet<PathBuf> = self.sources.iter().map(|(_, root)| root.clone()).collect();
        let watcher = self.watcher.clone();
        let roots = std::mem::take(&mut self.roots);
        self.roots = tokio::task::spawn_blocking(move || {
            let mut watcher = watcher.lock().unwrap();
            let mut roots = roots;
            roots.retain(|root| {
                if wanted.contains(root) {
                    return true;
                }
                if let Err(e) = watcher.unwatch(root) {
                    error!("Couldn't stop watching {}: {e}", root.display());
                }
                false
            });
            for root in wanted {
                if roots.contains(&root) {
                    continue;
                }
                match watcher.watch(&root, RecursiveMode::Recursive) {
                    Ok(()) => {
                        info!("Watching {} for changes", root.display());
                        roots.insert(root);
                    }
                    Err(e) => error!("Couldn't watch {}: {e}", root.display()),
                }
            }
            roots
        })
        .await
        .expect("watching panicked");
    }

    /// Puts off ingesting each data source whose files the event may have
    /// changed until they've gone unchanged for the quiet period.
    fn event(&mut self, event: notify::Result<Event>) {
        let event = match event {
            Ok(event) => event,
            Err(e) => {
                error!("Error watching data sources: {e}");
                return;
            }
        };
        let deadline = Instant::now() + self.quiet_period;
        // Events were lost, e.g. because too many came at once, so which files
        // are being written isn't known either.
        if event.need_rescan() {
            self.writing.clear();
            for (uuid, _) in &self.sources {
                self.due.insert(*uuid, deadline);
            }
            return;
        }
        self.track_writes(&event);
        if !may_change_files(&event.kind) {
            return;
        }
        for (uuid, root) in &self.sources {
            if event.paths.iter().any(|path| path.starts_with(root)) {
                self.due.insert(*uuid, deadline);
            }
        }
    }

    /// Keeps track of which files are being written: from a write until
    /// they're closed. Moving or linking a file in doesn't write it.
    fn track_writes(&mut self, event: &Event) {
        match event.kind {
            EventKind::Modify(ModifyKind::Data(_)) => {
                let now = Instant::now();
                for path in &event.paths {
                    self.writing.insert(path.clone(), now);
                }
            }
            EventKind::Access(AccessKind::Close(AccessMode::Write))
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                for path in &event.paths {
                    self.writing.remove(path);
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                if let [from, to] = &event.paths[..]
                    && let Some(written) = self.writing.remove(from)
                {
                    self.writing.insert(to.clone(), written);
                }
            }
            _ => {}
        }
    }

    /// If files under `root` are being written, when the last of them will
    /// count as stalled.
    fn writes_stall_at(&mut self, root: &Path, now: Instant) -> Option<Instant> {
        self.writing
            .retain(|_, written| *written + STALLED_WRITE > now);
        self.writing
            .iter()
            .filter(|(path, _)| path.starts_with(root))
            .map(|(_, written)| *written + STALLED_WRITE)
            .max()
    }

    async fn ingest_due(&mut self) {
        let now = Instant::now();
        let due: Vec<_> = self
            .due
            .iter()
            .filter(|&(_, &deadline)| deadline <= now)
            .map(|(&uuid, _)| uuid)
            .collect();
        for uuid in due {
            let root = self
                .sources
                .iter()
                .find(|(source, _)| *source == uuid)
                .map(|(_, root)| root.clone());
            if let Some(root) = root
                && let Some(stalls_at) = self.writes_stall_at(&root, now)
            {
                // Closing the file is a change, so it makes the data source
                // due again; failing that, it's ingested once writes stall.
                self.due.insert(uuid, stalls_at);
                continue;
            }
            self.due.remove(&uuid);
            if let Err(e) = start_ingest(&self.ctx, uuid).await {
                report_error!(e, "Couldn't ingest a data source whose files changed");
            }
        }
    }
}

/// Whether an event can change what an ingest measures. Reading a file isn't
/// a change, though it's reported as one when the file is opened: ingesting
/// reads new files, so it would otherwise go on ingesting forever, and a
/// media server playing from the library would keep it ingesting too.
fn may_change_files(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}
