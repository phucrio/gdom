//! Local-only GDOM logging: file sink plus mandatory secret redaction.
//!
//! Call [`init_file_logging`] once from the composition root. Emit events with
//! the `tracing` crate. Tokens, authorization codes, PKCE verifiers, and
//! client secrets are stripped before a line is written.

mod error;
mod init;
mod names;
mod redact;
mod rolling;
mod writer;

pub use error::LogError;
pub use init::{init_file_logging, log_file_path, LogGuard};
pub use names::{LOG_DIR_NAME, LOG_FILE_NAME, MAX_LOG_FILES, MAX_LOG_FILE_BYTES, REDACTED};
pub use redact::redact_secrets;
