//! The launcher's own log: `logs/cairn.log.<date>`, one file per day, 7 kept.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

use crate::{AppError, Paths, Result};

/// Install the global logger. Keep the guard alive for the whole program.
pub fn init_logging(paths: &Paths) -> Result<WorkerGuard> {
    std::fs::create_dir_all(paths.logs_dir())?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("cairn")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths.logs_dir())
        .map_err(|e| AppError::new("error.io").with_detail(e.to_string()))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_env("CAIRN_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
    Ok(guard)
}
