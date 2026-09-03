use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use url::Url;

use super::google_oauth::DesktopOAuthError;

const MAX_CALLBACK_BYTES: usize = 8 * 1024;

pub(super) struct Parameters {
    pub(super) state: String,
    pub(super) outcome: CallbackOutcome,
}

pub(super) enum CallbackOutcome {
    AuthorizationCode(String),
    ProviderError(String),
}

pub(super) async fn read_request(stream: &mut TcpStream) -> Result<String, DesktopOAuthError> {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        if request.len() == MAX_CALLBACK_BYTES {
            return Err(DesktopOAuthError::InvalidRequest);
        }
        let remaining = MAX_CALLBACK_BYTES - request.len();
        let read_capacity = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..read_capacity])
            .await
            .map_err(|_| DesktopOAuthError::InvalidRequest)?;
        if read == 0 {
            return Err(DesktopOAuthError::InvalidRequest);
        }
        request.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8(request).map_err(|_| DesktopOAuthError::InvalidRequest)
}

pub(super) fn parse(request: &str) -> Result<Parameters, DesktopOAuthError> {
    let mut request_line = request
        .lines()
        .next()
        .ok_or(DesktopOAuthError::InvalidRequest)?
        .split_whitespace();
    if request_line.next() != Some("GET") {
        return Err(DesktopOAuthError::InvalidRequest);
    }
    let target = request_line
        .next()
        .ok_or(DesktopOAuthError::InvalidRequest)?;
    if request_line.next() != Some("HTTP/1.1") || request_line.next().is_some() {
        return Err(DesktopOAuthError::InvalidRequest);
    }

    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| DesktopOAuthError::InvalidRequest)?;
    if url.path() != "/" {
        return Err(DesktopOAuthError::InvalidRequest);
    }

    let mut state = None;
    let mut code = None;
    let mut provider_error = None;
    for (name, value) in url.query_pairs() {
        let slot = match name.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut provider_error,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return Err(DesktopOAuthError::InvalidRequest);
        }
    }

    let state = state
        .filter(|value| !value.is_empty())
        .ok_or(DesktopOAuthError::InvalidRequest)?;
    let outcome = match (code, provider_error) {
        (Some(code), None) if !code.is_empty() => CallbackOutcome::AuthorizationCode(code),
        (None, Some(error)) if !error.is_empty() => CallbackOutcome::ProviderError(error),
        (None, None) | (Some(_), None) | (None, Some(_)) | (Some(_), Some(_)) => {
            return Err(DesktopOAuthError::InvalidRequest);
        }
    };

    Ok(Parameters { state, outcome })
}

pub(super) fn success_response() -> String {
    browser_response("200 OK", "Authorization complete. Return to GDOM.")
}

pub(super) fn error_response() -> String {
    browser_response("400 Bad Request", "Authorization failed. Return to GDOM.")
}

fn browser_response(status: &str, message: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
        message.len()
    )
}
