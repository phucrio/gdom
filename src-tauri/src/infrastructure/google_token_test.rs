use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use tokio::time::timeout;

use super::google_token::{GoogleTokenClient, GoogleTokenError};
use crate::infrastructure::google_oauth::OAuthGrant;

const CLIENT_ID: &str = "test-client-id.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "test-client-secret";
const AUTH_CODE: &str = "test-auth-code";
const PKCE_VERIFIER: &str = "test-pkce-verifier-string";
const REDIRECT_URI: &str = "http://127.0.0.1:8080";

fn grant() -> OAuthGrant {
    OAuthGrant::new(
        AUTH_CODE.to_owned(),
        PKCE_VERIFIER.to_owned(),
        REDIRECT_URI.to_owned(),
    )
}

fn serve_once(status: &str, body: &str) -> (String, Receiver<Result<String, String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server binds an ephemeral port");
    let address = listener.local_addr().expect("test server has an address");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let request = (|| -> std::io::Result<String> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;

            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk)?;
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
            }

            // Read the body if Content-Length is present
            let headers = String::from_utf8_lossy(&request);
            if let Some(content_length_line) = headers
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                && let Some(len_str) = content_length_line.split(':').nth(1)
                && let Ok(content_len) = len_str.trim().parse::<usize>()
            {
                let header_end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                let body_read = request.len() - header_end;
                if body_read < content_len {
                    let mut remaining = vec![0_u8; content_len - body_read];
                    stream.read_exact(&mut remaining)?;
                    request.extend_from_slice(&remaining);
                }
            }

            stream.write_all(response.as_bytes())?;
            String::from_utf8(request)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        })()
        .map_err(|error| error.to_string());

        drop(sender.send(request));
    });

    (format!("http://{address}"), receiver)
}

#[tokio::test]
async fn exchange_code_routes_correct_form_body_and_parses_tokens() {
    // Given
    let json_body = r#"{
        "access_token": "ya29.test-access-token",
        "expires_in": 3599,
        "refresh_token": "1//test-refresh-token",
        "scope": "https://www.googleapis.com/auth/drive",
        "token_type": "Bearer"
    }"#;
    let (base_url, request) = serve_once("200 OK", json_body);
    let client = GoogleTokenClient::for_test(
        base_url,
        CLIENT_ID.to_owned(),
        Some(CLIENT_SECRET.to_owned()),
    )
    .expect("test client builds");

    // When
    let tokens = timeout(Duration::from_secs(2), client.exchange_code(grant()))
        .await
        .expect("test completes before timeout")
        .expect("exchange succeeds");

    // Then
    assert_eq!(
        tokens.access_token.expose_secret(),
        "ya29.test-access-token"
    );
    assert_eq!(
        tokens.refresh_token.as_ref().map(|t| t.expose_secret()),
        Some("1//test-refresh-token")
    );
    assert_eq!(tokens.expires_in, Duration::from_secs(3599));

    let request = request
        .recv_timeout(Duration::from_secs(2))
        .expect("captured request is received")
        .expect("captured request is valid UTF-8");

    assert!(request.starts_with("POST /token HTTP/1.1\r\n"));
    assert!(request.contains("content-type: application/x-www-form-urlencoded"));
    assert!(request.contains(&format!("code={AUTH_CODE}")));
    assert!(request.contains(&format!("code_verifier={PKCE_VERIFIER}")));
    assert!(request.contains(&format!("client_id={CLIENT_ID}")));
    assert!(request.contains(&format!("client_secret={CLIENT_SECRET}")));
    assert!(request.contains("grant_type=authorization_code"));
}

#[tokio::test]
async fn exchange_code_maps_invalid_grant_error() {
    // Given
    let error_body = r#"{"error": "invalid_grant", "error_description": "Bad Request"}"#;
    let (base_url, _) = serve_once("400 Bad Request", error_body);
    let client = GoogleTokenClient::for_test(base_url, CLIENT_ID.to_owned(), None)
        .expect("test client builds");

    // When
    let error = timeout(Duration::from_secs(2), client.exchange_code(grant()))
        .await
        .expect("test completes before timeout")
        .expect_err("invalid grant produces error");

    // Then
    assert_eq!(error, GoogleTokenError::InvalidGrant);
}

#[tokio::test]
async fn exchange_code_maps_invalid_client_error() {
    // Given
    let error_body = r#"{"error": "invalid_client", "error_description": "Unauthorized"}"#;
    let (base_url, _) = serve_once("401 Unauthorized", error_body);
    let client = GoogleTokenClient::for_test(base_url, CLIENT_ID.to_owned(), None)
        .expect("test client builds");

    // When
    let error = timeout(Duration::from_secs(2), client.exchange_code(grant()))
        .await
        .expect("test completes before timeout")
        .expect_err("invalid client produces error");

    // Then
    assert_eq!(error, GoogleTokenError::InvalidClient);
}

#[tokio::test]
async fn exchange_code_maps_rate_limit_and_server_unavailable() {
    // Given
    for (status, expected) in [
        ("429 Too Many Requests", GoogleTokenError::RateLimited),
        (
            "500 Internal Server Error",
            GoogleTokenError::ServerUnavailable,
        ),
        (
            "503 Service Unavailable",
            GoogleTokenError::ServerUnavailable,
        ),
    ] {
        let (base_url, _) = serve_once(status, "{}");
        let client = GoogleTokenClient::for_test(base_url, CLIENT_ID.to_owned(), None)
            .expect("test client builds");

        // When
        let error = timeout(Duration::from_secs(2), client.exchange_code(grant()))
            .await
            .expect("test completes before timeout")
            .expect_err("status produces error");

        // Then
        assert_eq!(error, expected);
    }
}

#[tokio::test]
async fn token_response_and_errors_never_render_secrets() {
    // Given
    let secret = "sensitive-refresh-token-secret-12345";
    let json_body = format!(
        r#"{{"access_token": "secret-access-token", "expires_in": 3600, "refresh_token": "{secret}", "token_type": "Bearer"}}"#
    );
    let (base_url, _) = serve_once("200 OK", &json_body);
    let client = GoogleTokenClient::for_test(base_url, CLIENT_ID.to_owned(), None)
        .expect("test client builds");

    // When
    let tokens = timeout(Duration::from_secs(2), client.exchange_code(grant()))
        .await
        .expect("test completes before timeout")
        .expect("exchange succeeds");

    let tokens_debug = format!("{tokens:?}");

    // Then
    assert!(!tokens_debug.contains(secret));
    assert!(!tokens_debug.contains("secret-access-token"));
}
