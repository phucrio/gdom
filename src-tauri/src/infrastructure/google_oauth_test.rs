use std::collections::HashMap;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::google_oauth::{DesktopOAuthError, DesktopOAuthSession};

const CLIENT_ID: &str = "gdom-test.apps.googleusercontent.com";

fn query(url: &str) -> HashMap<String, String> {
    reqwest::Url::parse(url)
        .expect("authorization URL is valid")
        .query_pairs()
        .into_owned()
        .collect()
}

async fn send_callback(redirect_uri: &str, query: &str) -> String {
    let redirect = reqwest::Url::parse(redirect_uri).expect("redirect URI is valid");
    let address = format!(
        "{}:{}",
        redirect.host_str().expect("redirect URI has a host"),
        redirect.port().expect("redirect URI has an ephemeral port")
    );
    let mut stream = TcpStream::connect(address)
        .await
        .expect("callback client connects");
    stream
        .write_all(format!("GET /?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
        .await
        .expect("callback request writes");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("callback response reads");
    response
}

#[tokio::test]
async fn authorization_uses_loopback_pkce_and_full_drive_scope() {
    // Given / When
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let authorization_url =
        reqwest::Url::parse(session.authorization_url()).expect("authorization URL is valid");
    let parameters = query(session.authorization_url());

    // Then
    assert_eq!(authorization_url.scheme(), "https");
    assert_eq!(authorization_url.host_str(), Some("accounts.google.com"));
    assert_eq!(authorization_url.path(), "/o/oauth2/v2/auth");
    assert_eq!(
        parameters.get("client_id").map(String::as_str),
        Some(CLIENT_ID)
    );
    assert_eq!(
        parameters.get("response_type").map(String::as_str),
        Some("code")
    );
    assert_eq!(
        parameters.get("scope").map(String::as_str),
        Some("https://www.googleapis.com/auth/drive")
    );
    assert_eq!(
        parameters.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(
        parameters
            .get("code_challenge")
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        parameters
            .get("state")
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        parameters.get("access_type").map(String::as_str),
        Some("offline")
    );
    assert_eq!(
        parameters.get("prompt").map(String::as_str),
        Some("select_account")
    );
    assert_eq!(
        parameters.get("redirect_uri").map(String::as_str),
        Some(session.redirect_uri())
    );
    assert!(session.redirect_uri().starts_with("http://127.0.0.1:"));
}

#[tokio::test]
async fn callback_returns_a_redacted_grant_when_state_matches() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let authorization_code = "secret-authorization-code";
    let callback_query = format!("code={authorization_code}&state={state}");

    // When
    let (grant, response) = tokio::join!(
        session.receive_callback(),
        send_callback(&redirect_uri, &callback_query)
    );
    let grant = grant.expect("matching callback succeeds");

    // Then
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(grant.authorization_code(), authorization_code);
    assert!(!grant.pkce_verifier().is_empty());
    assert_eq!(grant.redirect_uri(), redirect_uri);
    let rendered = format!("{grant:?}");
    assert!(!rendered.contains(authorization_code));
    assert!(!rendered.contains(state));
}

#[tokio::test]
async fn callback_returns_access_denied_for_matching_provider_error() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let callback_query = format!("error=access_denied&state={state}");

    // When
    let (result, response) = tokio::join!(
        session.receive_callback(),
        send_callback(&redirect_uri, &callback_query)
    );

    // Then
    assert!(matches!(result, Err(DesktopOAuthError::AccessDenied)));
    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
}

#[tokio::test]
async fn callback_ignores_incomplete_or_conflicting_response_before_valid_redirect() {
    for invalid_suffix in ["", "code=forged&error=access_denied&"] {
        // Given
        let session = DesktopOAuthSession::start(CLIENT_ID)
            .await
            .expect("OAuth session starts");
        let parameters = query(session.authorization_url());
        let state = parameters
            .get("state")
            .expect("authorization URL contains state");
        let redirect_uri = session.redirect_uri().to_owned();
        let invalid_query = format!("{invalid_suffix}state={state}");
        let valid_query = format!("code=valid-code&state={state}");

        // When
        let callbacks = async {
            let invalid_response = send_callback(&redirect_uri, &invalid_query).await;
            let valid_response = send_callback(&redirect_uri, &valid_query).await;
            (invalid_response, valid_response)
        };
        let (grant, (invalid_response, valid_response)) =
            tokio::join!(session.receive_callback(), callbacks);

        // Then
        assert_eq!(
            grant.expect("valid callback succeeds").authorization_code(),
            "valid-code"
        );
        assert!(invalid_response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(valid_response.starts_with("HTTP/1.1 200 OK"));
    }
}

#[tokio::test]
async fn callback_rejects_duplicate_security_parameters() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let duplicate_query = format!("code=one&state={state}&state=forged");
    let valid_query = format!("code=valid-code&state={state}");

    // When
    let callbacks = async {
        let duplicate_response = send_callback(&redirect_uri, &duplicate_query).await;
        let valid_response = send_callback(&redirect_uri, &valid_query).await;
        (duplicate_response, valid_response)
    };
    let (grant, (duplicate_response, valid_response)) =
        tokio::join!(session.receive_callback(), callbacks);

    // Then
    assert_eq!(
        grant.expect("valid callback succeeds").authorization_code(),
        "valid-code"
    );
    assert!(duplicate_response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(valid_response.starts_with("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn callback_ignores_forged_connection_before_valid_redirect() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let valid_query = format!("code=valid-code&state={state}");

    // When
    let callbacks = async {
        let forged_response = send_callback(&redirect_uri, "code=forged&state=wrong").await;
        let valid_response = send_callback(&redirect_uri, &valid_query).await;
        (forged_response, valid_response)
    };
    let (grant, (forged_response, valid_response)) =
        tokio::join!(session.receive_callback(), callbacks);

    // Then
    assert_eq!(
        grant.expect("valid callback succeeds").authorization_code(),
        "valid-code"
    );
    assert!(forged_response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(valid_response.starts_with("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn callback_drops_stalled_connection_before_valid_redirect() {
    // Given
    let session = DesktopOAuthSession::start_for_test_with_timeouts(
        CLIENT_ID,
        Duration::from_secs(1),
        Duration::from_millis(10),
    )
    .await
    .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let valid_query = format!("code=valid-code&state={state}");

    // When
    let callbacks = async {
        let redirect = reqwest::Url::parse(&redirect_uri).expect("redirect URI is valid");
        let address = format!(
            "{}:{}",
            redirect.host_str().expect("redirect URI has a host"),
            redirect.port().expect("redirect URI has a port")
        );
        let stalled = TcpStream::connect(address)
            .await
            .expect("stalled callback connects");
        let valid_response = send_callback(&redirect_uri, &valid_query).await;
        drop(stalled);
        valid_response
    };
    let (grant, valid_response) = tokio::join!(session.receive_callback(), callbacks);

    // Then
    assert_eq!(
        grant.expect("valid callback succeeds").authorization_code(),
        "valid-code"
    );
    assert!(valid_response.starts_with("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn callback_wait_is_bounded() {
    // Given
    let session = DesktopOAuthSession::start_for_test(CLIENT_ID, Duration::ZERO)
        .await
        .expect("OAuth session starts");

    // When
    let result = session.receive_callback().await;

    // Then
    assert!(matches!(result, Err(DesktopOAuthError::Timeout)));
}
