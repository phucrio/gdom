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
pub use init::{LogGuard, init_file_logging, log_file_path};
pub use names::{LOG_DIR_NAME, LOG_FILE_NAME, MAX_LOG_FILE_BYTES, MAX_LOG_FILES, REDACTED};
pub use redact::redact_secrets;
