use crate::config::LoadConfigError;
use crate::interpret::knowledge_base::KnowledgeBaseError;
use crate::views::SyncError;
use error_stack::Report;
use std::fmt;
use thiserror::Error;
use wanshitong_core::{
    CoreError, CoreErrorContext,
    domain::{IngestUuid, InterpretationUuid, ViewUuid},
};

/// An application failure: a report of the [`AppErrorContext`] it ended in,
/// over the frames that led there, each with its location, and a span trace
/// and backtrace when captured. A failure from the core keeps the core's
/// frames, under [`AppErrorContext::Core`].
///
/// `Display` is the context's message alone; `{:#}` adds each cause after a
/// colon; `Debug` prints the whole tree. Log it with [`report_error!`] where
/// it's finally handled.
///
/// A newtype over the report so that it's a [`std::error::Error`], which `?`
/// into `anyhow` and topcoat's handlers need. It has no `source()`: the causes
/// are the report's frames, and a chain walker would print them again.
///
/// [`report_error!`]: crate::report_error
pub struct AppError(Report<AppErrorContext>);

impl AppError {
    /// Reports `context` as raised here.
    #[track_caller]
    pub fn new(context: AppErrorContext) -> Self {
        Self(Report::new(context))
    }

    /// What the failure ended in.
    pub fn context(&self) -> &AppErrorContext {
        self.0.current_context()
    }

    /// The core context the failure came from, if it did.
    pub fn core_context(&self) -> Option<&CoreErrorContext> {
        self.0.downcast_ref()
    }

    /// The whole report.
    pub fn report(&self) -> &Report<AppErrorContext> {
        &self.0
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl std::error::Error for AppError {}

impl<E> From<E> for AppError
where
    AppErrorContext: From<E>,
{
    /// Reports the error here, where it's raised or propagated with `?`.
    #[track_caller]
    fn from(error: E) -> Self {
        Self::new(error.into())
    }
}

impl From<CoreError> for AppError {
    /// Keeps the core's frames under [`AppErrorContext::Core`].
    #[track_caller]
    fn from(error: CoreError) -> Self {
        Self(error.change_context(AppErrorContext::Core))
    }
}

impl From<CoreErrorContext> for AppError {
    /// For raising a core context from the application, e.g. a not-found
    /// that the core would raise for the same record.
    #[track_caller]
    fn from(context: CoreErrorContext) -> Self {
        CoreError::from(context).into()
    }
}

/// The typed cause of an application failure. Each message describes its own
/// step only; a wrapped error is a source, so it's reported once, as its own
/// frame.
#[derive(Debug, Error)]
pub enum AppErrorContext {
    #[error(transparent)]
    Config(#[from] LoadConfigError),
    /// The frame below is the core's own context, which
    /// [`AppError::core_context`] finds.
    #[error("core failed")]
    Core,
    #[error(transparent)]
    KnowledgeBase(#[from] KnowledgeBaseError),
    #[error("couldn't watch the data sources")]
    Watch(#[from] notify::Error),
    /// Data sources an import names, by name, that this instance doesn't
    /// have.
    #[error(
        "no data source named {}; add {} with the same name, then import again",
        quoted(.0),
        if .0.len() == 1 { "it" } else { "them" }
    )]
    DataSourcesNotFound(Vec<String>),
    #[error("there are no assertions to import")]
    NothingToImport,
    #[error("ingest {0} not found")]
    IngestNotFound(IngestUuid),
    #[error("interpretation {0} not found")]
    InterpretationNotFound(InterpretationUuid),
    #[error("interpretation {0} hasn't succeeded, so it can't be canonical")]
    InterpretationNotSucceeded(InterpretationUuid),
    #[error("there is no catalog yet; ingest a data source first")]
    NoCatalog,
    #[error("no knowledge base is configured; set tmdb_token in config.toml")]
    NoKnowledgeBase,
    #[error("probe failed")]
    ProbeFailed(#[from] anyhow::Error),
    #[error("view {0} not found")]
    ViewNotFound(ViewUuid),
    #[error(transparent)]
    ViewSync(#[from] SyncError),
}

/// E.g. `"movies", "tv"`.
fn quoted(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Logs an error where it's finally handled, as one event with its message
/// (`Display`) in `error.message`, its full diagnostics (`Debug`: the causes,
/// their locations, the span trace and any backtrace) in `error.diagnostics`,
/// and whatever fields and message follow, as for [`tracing::error!`].
///
/// ```ignore
/// report_error!(error, view = %view.name, "Failed to refresh view");
/// ```
#[macro_export]
macro_rules! report_error {
    ($error:expr, $($fields:tt)+) => {
        match &$error {
            __report => $crate::__private::tracing::error!(
                error.message = %__report,
                error.diagnostics = ?__report,
                $($fields)+
            ),
        }
    };
}

#[doc(hidden)]
pub mod __private {
    pub use tracing;
}
