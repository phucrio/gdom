use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LogError {
    CreateDirectory { path: PathBuf, source: io::Error },
    Io(io::Error),
    AlreadyInitialized,
}

impl fmt::Display for LogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, source } => {
                write!(
                    formatter,
                    "could not create log directory {}: {source}",
                    path.display()
                )
            }
            Self::Io(error) => write!(formatter, "log I/O error: {error}"),
            Self::AlreadyInitialized => {
                formatter.write_str("tracing subscriber is already initialized")
            }
        }
    }
}

impl Error for LogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. } => Some(source),
            Self::Io(error) => Some(error),
            Self::AlreadyInitialized => None,
        }
    }
}

impl From<io::Error> for LogError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
