use super::*;

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
async fn callback_rejects_non_origin_form_target() {
    // Given
    let session = DesktopOAuthSession::start(CLIENT_ID)
        .await
        .expect("OAuth session starts");
    let parameters = query(session.authorization_url());
    let state = parameters
        .get("state")
        .expect("authorization URL contains state");
    let redirect_uri = session.redirect_uri().to_owned();
    let invalid_target = format!("@evil/?code=forged&state={state}");
    let valid_query = format!("code=valid-code&state={state}");

    // When
    let callbacks = async {
        let invalid_response = send_callback_target(&redirect_uri, &invalid_target).await;
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
