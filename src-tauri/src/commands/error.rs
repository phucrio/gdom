use std::fmt;

/// Sanitized error type for Tauri IPC commands.
///
/// Every variant carries a human-readable message that is safe to display
/// in the frontend. Tokens, secrets, and credential material must **never**
/// appear in the inner `String`; callers are responsible for sanitizing
/// before constructing a variant.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum CommandError {
    /// OAuth credentials have not been configured yet.
    NotConfigured(String),
    /// An error occurred during the OAuth flow.
    #[serde(rename = "oauth")]
    OAuth(String),
    /// Account validation failed (e.g. non-personal Google account).
    UnsupportedAccount(String),
    /// A database operation failed.
    Database(String),
    /// OS Keychain/Credential store operation failed.
    Keychain(String),
    /// The system browser could not be launched.
    BrowserLaunchFailed(String),
    /// Catch-all for unexpected internal errors.
    Internal(String),
    AccountNotFound(String),
    IdentityMismatch(String),
    ConfirmationRequired(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured(msg) => write!(f, "not configured: {msg}"),
            Self::OAuth(msg) => write!(f, "oauth error: {msg}"),
            Self::UnsupportedAccount(msg) => write!(f, "unsupported account: {msg}"),
            Self::Database(msg) => write!(f, "database error: {msg}"),
            Self::Keychain(msg) => write!(f, "keychain error: {msg}"),
            Self::BrowserLaunchFailed(msg) => write!(f, "browser launch failed: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
            Self::AccountNotFound(msg) => write!(f, "account not found: {msg}"),
            Self::IdentityMismatch(msg) => write!(f, "identity mismatch: {msg}"),
            Self::ConfirmationRequired(msg) => write!(f, "confirmation required: {msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<crate::application::AccountLifecycleError> for CommandError {
    fn from(err: crate::application::AccountLifecycleError) -> Self {
        use crate::application::AccountLifecycleError;
        match err {
            AccountLifecycleError::AccountNotFound => {
                Self::AccountNotFound("account not found".into())
            }
            AccountLifecycleError::IdentityMismatch { expected, actual } => {
                Self::IdentityMismatch(format!(
                    "expected google permission id {}, got {}",
                    expected.as_str(),
                    actual.as_str()
                ))
            }
            AccountLifecycleError::ActiveJobsPreventRemoval => {
                Self::Internal("cannot remove account with active jobs".into())
            }
            AccountLifecycleError::MissingRefreshToken => {
                Self::OAuth("missing refresh token".into())
            }
            AccountLifecycleError::TokenExchange(e) => Self::OAuth(e.to_string()),
            AccountLifecycleError::IdentityLookup(e) => Self::OAuth(e.to_string()),
            AccountLifecycleError::Account(e) => Self::UnsupportedAccount(e.to_string()),
            AccountLifecycleError::Database(e) => Self::Database(e.to_string()),
            AccountLifecycleError::Keychain(e) => Self::Keychain(e.to_string()),
        }
    }
}

impl From<crate::application::ConnectAccountError> for CommandError {
    fn from(err: crate::application::ConnectAccountError) -> Self {
        use crate::application::ConnectAccountError;
        match err {
            ConnectAccountError::TokenExchange(e) => Self::OAuth(e.to_string()),
            ConnectAccountError::IdentityLookup(e) => Self::OAuth(e.to_string()),
            ConnectAccountError::MissingRefreshToken => {
                Self::OAuth("Google did not return a refresh token".into())
            }
            ConnectAccountError::Account(e) => Self::UnsupportedAccount(e.to_string()),
            ConnectAccountError::Database(e) => Self::Database(e.to_string()),
            ConnectAccountError::Keychain(e) => Self::Keychain(e.to_string()),
            ConnectAccountError::RollbackFailed {
                primary_error,
                rollback_error,
            } => Self::Internal(format!(
                "connection failed ({primary_error}) and rollback also failed: {rollback_error}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_variant_prefix() {
        let cases: Vec<(CommandError, &str)> = vec![
            (
                CommandError::NotConfigured("missing client id".into()),
                "not configured: missing client id",
            ),
            (
                CommandError::OAuth("token exchange failed".into()),
                "oauth error: token exchange failed",
            ),
            (
                CommandError::UnsupportedAccount("workspace account".into()),
                "unsupported account: workspace account",
            ),
            (
                CommandError::Database("connection lost".into()),
                "database error: connection lost",
            ),
            (
                CommandError::Keychain("locked".into()),
                "keychain error: locked",
            ),
            (
                CommandError::BrowserLaunchFailed("no default browser".into()),
                "browser launch failed: no default browser",
            ),
            (
                CommandError::Internal("unexpected state".into()),
                "internal error: unexpected state",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn serialize_uses_tagged_camel_case() {
        let error = CommandError::NotConfigured("OAuth not set up".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "notConfigured");
        assert_eq!(json["message"], "OAuth not set up");
    }

    #[test]
    fn serialize_oauth_variant_uses_lowercase_tag() {
        let error = CommandError::OAuth("code exchange failed".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "oauth");
        assert_eq!(json["message"], "code exchange failed");
    }

    #[test]
    fn serialize_database_variant() {
        let error = CommandError::Database("migration failed".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "database");
        assert_eq!(json["message"], "migration failed");
    }

    #[test]
    fn serialize_keychain_variant() {
        let error = CommandError::Keychain("access denied".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "keychain");
        assert_eq!(json["message"], "access denied");
    }

    #[test]
    fn serialize_unsupported_account_variant() {
        let error = CommandError::UnsupportedAccount("only personal accounts".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "unsupportedAccount");
        assert_eq!(json["message"], "only personal accounts");
    }

    #[test]
    fn serialize_browser_launch_failed_variant() {
        let error = CommandError::BrowserLaunchFailed("cannot open URL".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "browserLaunchFailed");
        assert_eq!(json["message"], "cannot open URL");
    }

    #[test]
    fn serialize_internal_variant() {
        let error = CommandError::Internal("thread panic".into());
        let json = serde_json::to_value(&error).expect("serializes");

        assert_eq!(json["kind"], "internal");
        assert_eq!(json["message"], "thread panic");
    }

    #[test]
    fn debug_does_not_leak_tokens() {
        // Simulate a message that might accidentally contain a token-like string.
        // The error must not be constructed with real tokens — this test confirms
        // the debug representation only shows the sanitized message, not any
        // hidden fields.
        let error = CommandError::Internal("something went wrong".into());
        let debug = format!("{error:?}");

        assert!(debug.contains("something went wrong"));
        // No hidden fields that could leak credentials.
        assert!(!debug.contains("access_token"));
        assert!(!debug.contains("refresh_token"));
    }

    #[test]
    fn error_trait_is_implemented() {
        let error: Box<dyn std::error::Error> = Box::new(CommandError::Internal("test".into()));
        // Prove the trait object compiles and Display works through it.
        assert!(error.to_string().contains("test"));
    }
}
