mod assertions;
mod changeset;
mod claims;
mod cli;
mod client;
mod diff;
mod file;
mod kb;
mod marks;
mod text;
mod unidentified;
mod work;

use crate::cli::{Cli, Command, ServeArgs};
use anyhow::Context;
use clap::Parser;
use etcetera::{BaseStrategy, base_strategy::Xdg};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use wanshitong_web::{
    AppContext, AppError, Config, CoreContext, WebContext, apply_config, prune_interpretations,
    report_error, router, seed_data_sources, watch_data_sources,
};

#[tokio::main]
async fn main() -> ExitCode {
    let Cli { command } = Cli::parse();

    if !matches!(command, Command::Serve(_)) {
        exit_quietly_on_closed_stdout();
    }
    let result = match command {
        Command::Serve(args) => serve(args).await,
        Command::Unidentified(args) => unidentified::unidentified(args).await,
        Command::File(args) => file::file(args).await,
        Command::Work(args) => work::work(args).await,
        Command::Kb(command) => kb::kb(command).await,
        Command::Changeset(command) => changeset::changeset(command).await,
        Command::Diff(args) => diff::diff(args).await,
        Command::Assert(args) => changeset::assert(args).await,
        Command::Retract(args) => changeset::retract(args).await,
        Command::Marks(args) => marks::marks(args).await,
        Command::Assertions(command) => assertions::assertions(command).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // An application error's causes are in its report, not in a chain
            // that anyhow could print, so the report is printed instead.
            match error.downcast_ref::<AppError>() {
                Some(app_error) => eprintln!("Error: {app_error:?}"),
                None => eprintln!("Error: {error:?}"),
            }
            ExitCode::FAILURE
        }
    }
}

/// Lets a client command end quietly when what it prints to stops reading, e.g.
/// `wst work ... | head`, as other command-line tools do. Rust ignores SIGPIPE,
/// so a write to a closed pipe fails, and `println!` panics. The server keeps
/// ignoring it, so that a client hanging up mid-response doesn't kill it.
fn exit_quietly_on_closed_stdout() {
    #[cfg(unix)]
    // SAFETY: nothing else is handling signals yet; this restores the default.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn setup_logging() {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info,tower_http=debug"))
        .unwrap();
    // The error layer keeps the span trace that error reports capture.
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(ErrorLayer::default());
    // Reports are multi-line; the pretty format gives each event room for
    // them. The compact format is for a log that something else collects.
    if std::io::stdout().is_terminal() {
        subscriber
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    } else {
        subscriber.with(tracing_subscriber::fmt::layer()).init();
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let ServiceDirs {
        config_dir,
        data_dir,
        cache_dir,
    } = ServiceDirs::resolve(args)?;

    setup_logging();
    let config = Config::load(&config_dir)?;
    info!(
        "Config in {}, data in {}, cache in {}",
        config_dir.display(),
        data_dir.display(),
        cache_dir.display()
    );

    let Config {
        host,
        port,
        watch_data_sources: watch,
        ..
    } = config;

    // A direct call, not `map_err(AppError::from)`: passed as a value, the
    // conversion loses the location it would record for the report.
    let core = match CoreContext::new_in_file(&data_dir).await {
        Ok(core) => core,
        Err(error) => return Err(AppError::from(error).into()),
    };
    let app = AppContext::new(core, config, cache_dir);
    apply_config(&app).await?;
    seed_data_sources(&app).await?;
    if let Err(e) = prune_interpretations(&app).await {
        report_error!(e, "Couldn't prune old interpretations");
    }
    let web = WebContext::new(app.clone());

    let addr = SocketAddr::new(host, port);
    let listener = TcpListener::bind(addr).await?;
    info!("Listening at http://{addr}",);
    // Watching walks every data source's tree, which takes a minute when the
    // disks are cold, so it runs behind the listener rather than before it:
    // a reverse proxy answers 502 until the port is bound. Data sources can
    // still be ingested by hand.
    if watch {
        tokio::spawn(async move {
            if let Err(e) = watch_data_sources(&app).await {
                report_error!(e, "Couldn't watch the data sources");
            }
        });
    }
    axum::serve(listener, router(web.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("HTTP service shut down");

    web.shutdown().await?;

    info!("All application tasks exited");

    Ok(())
}

/// Where the service keeps its files, each created if missing.
struct ServiceDirs {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl ServiceDirs {
    /// The directories `wst serve` was given, each defaulting to `wst/` under
    /// the XDG base directory of its kind. The home directory is only needed
    /// for the defaults, so giving all three works without one.
    fn resolve(args: ServeArgs) -> anyhow::Result<Self> {
        let ServeArgs {
            config_dir,
            data_dir,
            cache_dir,
        } = args;
        let home = Xdg::new();
        let dir = |given: Option<PathBuf>,
                   flag: &str,
                   base: fn(&Xdg) -> PathBuf|
         -> anyhow::Result<PathBuf> {
            let dir = match given {
                Some(dir) => dir,
                None => base(home.as_ref().map_err(|e| {
                    anyhow::anyhow!("{e}; pass {flag} to say where the directory is")
                })?)
                .join("wst"),
            };
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            Ok(dir)
        };
        Ok(Self {
            config_dir: dir(config_dir, "--config-dir", BaseStrategy::config_dir)?,
            data_dir: dir(data_dir, "--data-dir", BaseStrategy::data_dir)?,
            cache_dir: dir(cache_dir, "--cache-dir", BaseStrategy::cache_dir)?,
        })
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Got shutdown signal");
}
