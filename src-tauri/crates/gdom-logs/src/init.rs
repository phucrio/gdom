use std::fs;
use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
#[cfg(debug_assertions)]
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

use crate::error::LogError;
use crate::names::{DEFAULT_ENV_FILTER, LOG_FILE_NAME};
use crate::rolling::SizeRollingFile;
use crate::writer::RedactingMakeWriter;

/// Keeps the background log worker alive. Dropping this stops file writes.
#[must_use = "dropping LogGuard stops the file log worker"]
pub struct LogGuard {
    _worker: WorkerGuard,
}

pub fn log_file_path(log_dir: &Path) -> PathBuf {
    log_dir.join(LOG_FILE_NAME)
}

pub fn init_file_logging(log_dir: impl AsRef<Path>) -> Result<LogGuard, LogError> {
    let log_dir = log_dir.as_ref();
    fs::create_dir_all(log_dir).map_err(|source| LogError::CreateDirectory {
        path: log_dir.to_path_buf(),
        source,
    })?;

    let rolling_file = SizeRollingFile::open(log_dir)?;
    let (non_blocking, worker) = tracing_appender::non_blocking(rolling_file);
    let file_writer = RedactingMakeWriter::new(non_blocking);
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::new(DEFAULT_ENV_FILTER),
    };

    #[cfg(debug_assertions)]
    let result = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(env_filter)
        .with_writer(file_writer.and(RedactingMakeWriter::new(std::io::stderr)))
        .try_init();

    #[cfg(not(debug_assertions))]
    let result = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(env_filter)
        .with_writer(file_writer)
        .try_init();

    match result {
        Ok(()) => Ok(LogGuard { _worker: worker }),
        Err(_) => Err(LogError::AlreadyInitialized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn log_file_path_uses_stable_file_name() {
        let dir = PathBuf::from("C:/tmp/gdom-logs");
        assert_eq!(log_file_path(&dir), dir.join(LOG_FILE_NAME));
    }

    #[test]
    fn init_creates_the_log_directory() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("gdom-logs-init-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        match init_file_logging(&dir) {
            Ok(_guard) => {
                assert!(dir.is_dir());
            }
            Err(LogError::AlreadyInitialized) => {
                fs::create_dir_all(&dir).expect("log dir");
                assert!(dir.is_dir());
            }
            Err(error) => panic!("init failed: {error}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
