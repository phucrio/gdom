use super::*;

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
        Some("consent select_account")
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
