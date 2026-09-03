use std::collections::HashMap;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use super::google_oauth::{DesktopOAuthError, DesktopOAuthSession, MAX_IN_FLIGHT_CALLBACKS};

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
async fn callback_returns_provider_failure_for_other_google_errors() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let callback_query = format!("error=temporarily_unavailable&state={state}");

    // When
    let (result, response) = tokio::join!(
        session.receive_callback(),
        send_callback(&redirect_uri, &callback_query)
    );

    // Then
    assert!(matches!(&result, Err(DesktopOAuthError::ProviderFailure)));
    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(!format!("{result:?}").contains("temporarily_unavailable"));
}

#[tokio::test]
async fn callback_ignores_incomplete_or_conflicting_response_before_valid_redirect() {
    for invalid_suffix in [
        "",
        "code=forged&error=access_denied&",
        "code=&",
        "error=&",
        "code=&error=access_denied&",
        "code=forged&error=&",
    ] {
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
async fn callback_ignores_empty_state_before_valid_redirect() {
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
        let invalid_response = send_callback(&redirect_uri, "code=forged&state=").await;
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
        Duration::from_secs(3),
        Duration::from_secs(2),
    )
    .await
    .expect("OAuth session starts");
    let (session, mut handler_started) = session.notify_handler_started_for_test();
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let valid_query = format!("code=valid-code&state={state}");

    let redirect = reqwest::Url::parse(&redirect_uri).expect("redirect URI is valid");
    let address = format!(
        "{}:{}",
        redirect.host_str().expect("redirect URI has a host"),
        redirect.port().expect("redirect URI has a port")
    );
    let mut stalled = Vec::new();
    for _ in 0..MAX_IN_FLIGHT_CALLBACKS {
        stalled.push(
            TcpStream::connect(&address)
                .await
                .expect("stalled callback connects"),
        );
    }

    // When
    let callbacks = async {
        tokio::time::timeout(Duration::from_millis(200), async {
            for _ in 0..MAX_IN_FLIGHT_CALLBACKS {
                handler_started
                    .recv()
                    .await
                    .expect("handler notification channel stays open");
            }
        })
        .await
        .expect("all stalled callback handlers start");
        let overload_rejected = tokio::time::timeout(Duration::from_millis(200), async {
            let mut stream = TcpStream::connect(&address)
                .await
                .expect("excess callback connects");
            let write_result = stream.write_all(b"GET / HTTP/1.1\r\n").await;
            let mut response = [0];
            match stream.read(&mut response).await {
                Ok(0) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::ConnectionReset
                    ) => {}
                read_result => {
                    panic!("excess callback remained open: {read_result:?}, {write_result:?}")
                }
            }
        })
        .await;
        assert!(
            overload_rejected.is_ok(),
            "excess callback must be closed without waiting for a handler"
        );
        let mut released = stalled.pop().expect("a stalled callback is available");
        released
            .write_all(b"GET /?code=forged&state=wrong HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .expect("stalled callback completes its request");
        let mut rejected_response = String::new();
        released
            .read_to_string(&mut rejected_response)
            .await
            .expect("completed stalled callback reads its rejection");
        assert!(rejected_response.starts_with("HTTP/1.1 400 Bad Request"));
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
async fn callback_preserves_grant_when_response_write_fails() {
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
    let session = session.fail_response_write_for_test();

    // When
    let (grant, response) = tokio::join!(
        session.receive_callback(),
        send_callback(&redirect_uri, &callback_query)
    );
    let grant = grant.expect("matching callback survives response write failure");

    // Then
    assert_eq!(grant.authorization_code(), authorization_code);
    assert!(response.is_empty());
}

#[tokio::test]
async fn callback_preserves_grant_when_response_write_crosses_session_deadline() {
    // Given
    let session = DesktopOAuthSession::start_for_test_with_timeouts(
        CLIENT_ID,
        Duration::from_secs(10),
        Duration::from_secs(20),
    )
    .await
    .expect("OAuth session starts");
    let (session, mut handler_started) = session.notify_handler_started_for_test();
    let (session, mut response_write_started) =
        session.delay_response_write_for_test(Duration::from_secs(2));
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect = reqwest::Url::parse(session.redirect_uri()).expect("redirect URI is valid");
    let address = format!(
        "{}:{}",
        redirect.host_str().expect("redirect URI has a host"),
        redirect.port().expect("redirect URI has a port")
    );
    let request =
        format!("GET /?code=near-deadline&state={state} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    let (send_request, receive_request) = oneshot::channel();

    // When
    let browser = async {
        let mut stream = TcpStream::connect(address)
            .await
            .expect("callback client connects");
        receive_request.await.expect("callback request is released");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("callback request writes");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .expect("callback response reads");
        response
    };
    let clock = async {
        handler_started
            .recv()
            .await
            .expect("callback handler starts");
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(9)).await;
        send_request.send(()).expect("callback request releases");
        let mut response_started = false;
        for _ in 0..1_000 {
            match response_write_started.try_recv() {
                Ok(()) => {
                    response_started = true;
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    panic!("response write notification disconnected")
                }
            }
        }
        assert!(response_started, "response write starts");
        tokio::time::advance(Duration::from_secs(2)).await;
    };
    let (grant, response, ()) = tokio::join!(session.receive_callback(), browser, clock);

    // Then
    assert_eq!(
        grant
            .expect("near-deadline callback succeeds")
            .authorization_code(),
        "near-deadline"
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn callback_wait_is_bounded() {
    // Given
    let session = DesktopOAuthSession::start_for_test_with_timeouts(
        CLIENT_ID,
        Duration::ZERO,
        Duration::from_secs(2),
    )
    .await
    .expect("OAuth session starts");

    // When
    let result = session.receive_callback().await;

    // Then
    assert!(matches!(result, Err(DesktopOAuthError::Timeout)));
}
