use std::fmt;

/// Desktop OAuth client extracted from a Google Cloud Console JSON download.
///
/// The client secret is redacted from `Debug` output so it cannot leak into logs.
#[derive(Clone, Eq, PartialEq)]
pub struct DesktopOAuthClient {
    pub client_id: String,
    pub client_secret: String,
}

impl fmt::Debug for DesktopOAuthClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopOAuthClient")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoogleClientJsonError {
    InvalidJson,
    NotDesktopClient,
    MissingClientId,
    MissingClientSecret,
}

impl fmt::Display for GoogleClientJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => formatter.write_str("file is not valid OAuth client JSON"),
            Self::NotDesktopClient => formatter.write_str(
                "file is not a Google Desktop (installed) OAuth client. Create a Desktop app client in Google Cloud Console and download its JSON",
            ),
            Self::MissingClientId => formatter.write_str("Desktop client JSON is missing client_id"),
            Self::MissingClientSecret => {
                formatter.write_str("Desktop client JSON is missing client_secret")
            }
        }
    }
}

impl std::error::Error for GoogleClientJsonError {}

#[derive(serde::Deserialize)]
struct GoogleClientFile {
    installed: Option<GoogleClientFields>,
    web: Option<GoogleClientFields>,
}

#[derive(serde::Deserialize)]
struct GoogleClientFields {
    client_id: Option<String>,
    client_secret: Option<String>,
}

/// Parse the JSON Google Cloud Console emits for a Desktop ("installed") client.
pub fn parse_desktop_client_json(
    bytes: &[u8],
) -> Result<DesktopOAuthClient, GoogleClientJsonError> {
    let parsed: GoogleClientFile =
        serde_json::from_slice(bytes).map_err(|_| GoogleClientJsonError::InvalidJson)?;
    let fields = match (parsed.installed, parsed.web) {
        (Some(installed), _) => installed,
        (None, Some(_)) => return Err(GoogleClientJsonError::NotDesktopClient),
        (None, None) => return Err(GoogleClientJsonError::NotDesktopClient),
    };
    let client_id = fields
        .client_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(GoogleClientJsonError::MissingClientId)?
        .to_owned();
    let client_secret = fields
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(GoogleClientJsonError::MissingClientSecret)?
        .to_owned();
    Ok(DesktopOAuthClient {
        client_id,
        client_secret,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_installed_desktop_client() {
        let json = br#"{
            "installed": {
                "client_id": "abc.apps.googleusercontent.com",
                "project_id": "gdom",
                "client_secret": "GOCSPX-test-secret",
                "redirect_uris": ["http://localhost"]
            }
        }"#;
        let client = parse_desktop_client_json(json).expect("parses desktop client");
        assert_eq!(client.client_id, "abc.apps.googleusercontent.com");
        assert_eq!(client.client_secret, "GOCSPX-test-secret");
    }

    #[test]
    fn parse_rejects_web_client() {
        let json = br#"{
            "web": {
                "client_id": "web.apps.googleusercontent.com",
                "client_secret": "web-secret"
            }
        }"#;
        assert_eq!(
            parse_desktop_client_json(json),
            Err(GoogleClientJsonError::NotDesktopClient)
        );
    }

    #[test]
    fn parse_rejects_missing_secret() {
        let json = br#"{
            "installed": {
                "client_id": "abc.apps.googleusercontent.com",
                "client_secret": "  "
            }
        }"#;
        assert_eq!(
            parse_desktop_client_json(json),
            Err(GoogleClientJsonError::MissingClientSecret)
        );
    }

    #[test]
    fn parse_rejects_invalid_json() {
        assert_eq!(
            parse_desktop_client_json(b"not-json"),
            Err(GoogleClientJsonError::InvalidJson)
        );
    }

    #[test]
    fn debug_redacts_client_secret() {
        let client = DesktopOAuthClient {
            client_id: "id".into(),
            client_secret: "GOCSPX-must-not-leak".into(),
        };
        let debug = format!("{client:?}");
        assert!(debug.contains("id"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("GOCSPX-must-not-leak"));
    }
}
