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
        ("404 Not Found", GoogleDriveError::NotFound),
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

#[tokio::test]
async fn get_folder_metadata_parses_fields_and_owners() {
    let body = r#"{
        "id": "folder-123",
        "name": "Important Folder",
        "mimeType": "application/vnd.google-apps.folder",
        "trashed": false,
        "owners": [
            {
                "permissionId": "perm-owner-1",
                "emailAddress": "owner@gmail.com"
            }
        ]
    }"#;

    let (base_url, request) = serve_once("200 OK", body);
    let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
    let token = AccessToken::new(SECRET.to_owned());

    let meta = client
        .get_folder_metadata(&token, "folder-123")
        .await
        .expect("folder metadata loaded");

    assert_eq!(meta.id, "folder-123");
    assert_eq!(meta.name, "Important Folder");
    assert_eq!(meta.mime_type, "application/vnd.google-apps.folder");
    assert!(!meta.trashed);
    assert_eq!(meta.drive_id, None);
    assert_eq!(meta.owners.len(), 1);
    assert_eq!(meta.owners[0].permission_id.as_str(), "perm-owner-1");
    assert_eq!(
        meta.owners[0].email_address.as_deref(),
        Some("owner@gmail.com")
    );

    let request = request
        .recv_timeout(Duration::from_secs(2))
        .expect("test server captures the request")
        .expect("valid UTF-8");
    assert!(request.contains("GET /drive/v3/files/folder-123?supportsAllDrives=true&fields="));
}

#[tokio::test]
async fn get_folder_metadata_maps_not_found() {
    let (base_url, _) = serve_once("404 Not Found", "{}");
    let client = GoogleDriveClient::for_test(base_url).expect("test client builds");
    let token = AccessToken::new(SECRET.to_owned());

    let err = client
        .get_folder_metadata(&token, "nonexistent")
        .await
        .expect_err("404 error returned");

    assert_eq!(err, GoogleDriveError::NotFound);
}

fn serve_sequence(
    responses: Vec<(String, String)>,
) -> (String, Receiver<Result<Vec<String>, String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server binds an ephemeral port");
    let address = listener.local_addr().expect("test server has an address");
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body) in responses {
            let request = (|| -> std::io::Result<String> {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                let mut request = Vec::new();
                let mut chunk = [0_u8; 2048];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk)?;
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                }
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes())?;
                String::from_utf8(request)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            })()
            .map_err(|error| error.to_string());
            match request {
                Ok(value) => captured.push(value),
                Err(error) => {
                    drop(sender.send(Err(error)));
                    return;
                }
            }
        }
        drop(sender.send(Ok(captured)));
    });

    (format!("http://{address}"), receiver)
}

#[tokio::test]
async fn list_children_paginates_and_sends_source_bearer() {
    let page_one = r#"{"nextPageToken":"token-2","files":[{"id":"a","name":"A","mimeType":"text/plain","owners":[{"permissionId":"p1"}]}]}"#;
    let page_two = r#"{"files":[{"id":"b","name":"B","mimeType":"application/vnd.google-apps.shortcut","shortcutDetails":{"targetId":"target-z"},"owners":[{"permissionId":"p1"}]}]}"#;
    let (base_url, requests) = serve_sequence(vec![
        ("200 OK".into(), page_one.into()),
        ("200 OK".into(), page_two.into()),
    ]);
    let client = GoogleDriveClient::for_test(base_url).unwrap();
    let token = AccessToken::new(SECRET.to_owned());

    let first = client
        .list_children(&token, "folder-root", None)
        .await
        .unwrap();
    assert_eq!(first.next_page_token.as_deref(), Some("token-2"));
    assert_eq!(first.files[0].id, "a");

    let second = client
        .list_children(&token, "folder-root", Some("token-2"))
        .await
        .unwrap();
    assert!(second.next_page_token.is_none());
    assert_eq!(
        second.files[0].shortcut_target_id.as_deref(),
        Some("target-z")
    );

    let captured = requests
        .recv_timeout(Duration::from_secs(2))
        .expect("captured")
        .expect("utf8");
    assert!(captured[0].contains("pageSize=1000"));
    assert!(captured[0].contains("spaces=drive"));
    assert!(captured[0].contains("supportsAllDrives=true"));
    assert!(captured[0].contains("trashed%3Dfalse") || captured[0].contains("trashed=false"));
    assert!(captured[1].contains("pageToken=token-2"));
    for request in &captured {
        assert!(request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == format!("Bearer {SECRET}")
            })
        }));
    }
}

#[tokio::test]
async fn list_children_maps_rate_limit() {
    let (base_url, _) = serve_once("429 Too Many Requests", "{}");
    let client = GoogleDriveClient::for_test(base_url).unwrap();
    let token = AccessToken::new(SECRET.to_owned());
    let err = client
        .list_children(&token, "folder", None)
        .await
        .expect_err("429");
    assert_eq!(err, GoogleDriveError::RateLimited);
}

#[tokio::test]
async fn storage_quota_uses_caller_bearer_and_parses_limit() {
    let body = r#"{"storageQuota":{"limit":"5000","usage":"120"}}"#;
    let (base_url, request) = serve_once("200 OK", body);
    let client = GoogleDriveClient::for_test(base_url).unwrap();
    let token = AccessToken::new("target-secret-token".into());
    let quota = client.storage_quota(&token).await.unwrap();
    assert_eq!(quota.limit_bytes, Some(5000));
    assert_eq!(quota.usage_bytes, 120);
    let request = request
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert!(request.contains("GET /drive/v3/about?fields=storageQuota"));
    assert!(request.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                && value.trim() == "Bearer target-secret-token"
        })
    }));
}

#[tokio::test]
async fn create_pending_owner_posts_source_body_without_parent_rewrite() {
    let body = r#"{"id":"perm-created","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}"#;
    let (base_url, request) = serve_once("200 OK", body);
    let client = GoogleDriveClient::for_test(base_url).expect("test client");
    let token = AccessToken::new(SECRET.to_owned());
    let permission = timeout(
        Duration::from_secs(2),
        client.create_pending_owner(&token, "file-1", "target@gmail.com"),
    )
    .await
    .expect("completes")
    .expect("pending owner created");
    assert_eq!(permission.id, "perm-created");
    assert!(permission.pending_owner);

    let request = request
        .recv_timeout(Duration::from_secs(2))
        .expect("captured")
        .expect("utf8");
    assert!(request.starts_with("POST /drive/v3/files/file-1/permissions?"));
    assert!(request.contains("sendNotificationEmail=true"));
    assert!(!request.contains("sendNotificationEmail=false"));
    assert!(!request.contains("moveToNewOwnersRoot"));
    assert!(request.contains("Bearer"));
}

#[tokio::test]
async fn accept_ownership_patches_with_transfer_flag() {
    let body = r#"{"id":"perm-created","role":"owner","pendingOwner":false}"#;
    let (base_url, request) = serve_once("200 OK", body);
    let client = GoogleDriveClient::for_test(base_url).expect("test client");
    let token = AccessToken::new(SECRET.to_owned());
    timeout(
        Duration::from_secs(2),
        client.accept_ownership(&token, "file-1", "perm-created"),
    )
    .await
    .expect("completes")
    .expect("accepted");
    let request = request
        .recv_timeout(Duration::from_secs(2))
        .expect("captured")
        .expect("utf8");
    assert!(request.starts_with("PATCH /drive/v3/files/file-1/permissions/perm-created?"));
    assert!(request.contains("transferOwnership=true"));
    assert!(!request.contains("moveToNewOwnersRoot=true"));
}

#[tokio::test]
async fn sharing_rate_limit_reason_is_distinct_from_generic_forbidden() {
    let body = r#"{"error":{"errors":[{"reason":"sharingRateLimitExceeded"}]}}"#;
    let (base_url, _) = serve_once("403 Forbidden", body);
    let client = GoogleDriveClient::for_test(base_url).expect("test client");
    let token = AccessToken::new(SECRET.to_owned());
    let error = timeout(
        Duration::from_secs(2),
        client.create_pending_owner(&token, "file-1", "target@gmail.com"),
    )
    .await
    .expect("completes")
    .expect_err("sharing rate limit");
    assert_eq!(error, GoogleDriveError::SharingRateLimitExceeded);
}
