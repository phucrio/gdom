use super::*;

#[tokio::test]
async fn callback_replaces_oldest_stalled_handler_for_valid_redirect() {
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
        tokio::time::timeout(Duration::from_secs(1), response_write_started.recv())
            .await
            .expect("response write notification arrives before deadline")
            .expect("response write notification channel stays open");
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
