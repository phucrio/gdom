pub const LOG_DIR_NAME: &str = "logs";
pub const LOG_FILE_NAME: &str = "gdom.log";
pub const REDACTED: &str = "[REDACTED]";
pub(crate) const DEFAULT_ENV_FILTER: &str = "info";
pub const MAX_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_LOG_FILES: u32 = 5;

pub(crate) const SECRET_KEYS: &[&str] = &[
    "access_token",
    "accesstoken",
    "refresh_token",
    "refreshtoken",
    "id_token",
    "idtoken",
    "client_secret",
    "clientsecret",
    "code_verifier",
    "codeverifier",
    "pkce_verifier",
    "pkceverifier",
    "authorization_code",
    "authorizationcode",
    "pkce",
];

pub(crate) const BEARER_PREFIX: &str = "bearer ";
pub(crate) const GOOGLE_ACCESS_PREFIX: &str = "ya29.";
pub(crate) const GOOGLE_REFRESH_PREFIX: &str = "1//";
pub(crate) const GOOGLE_AUTH_CODE_PREFIX: &str = "code=4/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_is_ten_megabytes_times_five_files() {
        assert_eq!(MAX_LOG_FILE_BYTES, 10 * 1024 * 1024);
        assert_eq!(MAX_LOG_FILES, 5);
        assert_eq!(LOG_FILE_NAME, "gdom.log");
        assert_eq!(LOG_DIR_NAME, "logs");
    }
}
