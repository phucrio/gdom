use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use tokio::time::timeout;

use crate::application::AccessToken;

use super::google_drive::{DriveAccountIdentity, GoogleDriveClient, GoogleDriveError};

const SECRET: &str = "wire-secret-access-token";

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

            stream.write_all(response.as_bytes())?;
            String::from_utf8(request)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        })()
        .map_err(|error| error.to_string());

        drop(sender.send(request));
    });

    (format!("http://{address}"), receiver)
}

async fn identify(
    client: &GoogleDriveClient,
    token: &AccessToken,
) -> Result<DriveAccountIdentity, GoogleDriveError> {
    timeout(Duration::from_secs(2), client.account_identity(token))
        .await
        .expect("Drive request completes before the test timeout")
}

#[tokio::test]
async fn about_get_routes_bearer_token_and_parses_identity() {
    // Given
    let body = r#"{"user":{"permissionId":"permission-123","emailAddress":"owner@example.com","displayName":"Owner"}}"#;
    let (base_url, request) = serve_once("200 OK", body);
    let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
    let token = AccessToken::new(SECRET.to_owned());

    // When
    let identity = identify(&client, &token)
        .await
        .expect("account identity loads");

    // Then
    assert_eq!(identity.permission_id().as_str(), "permission-123");
    assert_eq!(identity.email(), "owner@example.com");
    assert_eq!(identity.display_name(), "Owner");

    let request = request
        .recv_timeout(Duration::from_secs(2))
        .expect("test server captures the request")
        .expect("captured request is valid UTF-8");
    assert!(request.starts_with(
        "GET /drive/v3/about?fields=user%28permissionId%2CemailAddress%2CdisplayName%29 HTTP/1.1\r\n"
    ));
    assert!(request.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value.trim() == format!("Bearer {SECRET}")
        })
    }));
    assert!(request.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("user-agent") && value.trim().starts_with("gdom/")
        })
    }));
}

#[tokio::test]
async fn about_get_maps_caller_actionable_statuses() {
    // Given
    let cases = [
        ("401 Unauthorized", GoogleDriveError::Unauthorized),
        ("403 Forbidden", GoogleDriveError::Forbidden),
        ("429 Too Many Requests", GoogleDriveError::RateLimited),
        (
            "503 Service Unavailable",
            GoogleDriveError::ServerUnavailable,
        ),
        ("418 I'm a teapot", GoogleDriveError::UnexpectedStatus(418)),
    ];

    for (status, expected) in cases {
        let (base_url, _) = serve_once(status, "{}");
        let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
        let token = AccessToken::new(SECRET.to_owned());

        // When
        let error = identify(&client, &token)
            .await
            .expect_err("non-success status is rejected");

        // Then
        assert_eq!(error, expected);
    }
}

#[tokio::test]
async fn about_get_rejects_malformed_json() {
    // Given
    let (base_url, _) = serve_once("200 OK", "not-json");
    let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
    let token = AccessToken::new(SECRET.to_owned());

    // When
    let error = identify(&client, &token)
        .await
        .expect_err("malformed JSON is rejected");

    // Then
    assert_eq!(error, GoogleDriveError::InvalidResponse);
}

#[tokio::test]
async fn access_token_and_drive_errors_never_render_secrets() {
    // Given
    let (base_url, _) = serve_once("401 Unauthorized", SECRET);
    let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
    let token = AccessToken::new(SECRET.to_owned());

    // When
    let token_debug = format!("{token:?}");
    let token_display = token.to_string();
    let error = identify(&client, &token)
        .await
        .expect_err("unauthorized response is rejected");
    let error_debug = format!("{error:?}");
    let error_display = error.to_string();

    // Then
    for rendered in [token_debug, token_display, error_debug, error_display] {
        assert!(!rendered.contains(SECRET));
    }
}
