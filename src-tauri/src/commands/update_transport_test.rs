use std::{net::TcpListener, time::Duration};

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri_plugin_updater::{Updater, UpdaterExt};

use super::policy::is_newer_stable;
use crate::test_support::spawn_http_handler;

const ARTIFACT: &str = include_str!("update_fixtures/artifact.txt");
const SIGNATURE: &str = include_str!("update_fixtures/artifact.txt.sig");
const PUBLIC_KEY: &str = include_str!("update_fixtures/public-key.txt");

fn updater(endpoint: &str, target: &str) -> Updater {
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({
            "pubkey": PUBLIC_KEY.trim(),
            "endpoints": [endpoint],
            "dangerousInsecureTransportProtocol": true
        }),
    );
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("mock app");
    app.updater_builder()
        .executable_path(std::env::temp_dir().join("Gdom.app/Contents/MacOS/gdom"))
        .target(target)
        .no_proxy()
        .timeout(Duration::from_millis(250))
        .version_comparator(|current, offered| is_newer_stable(&current, &offered.version))
        .build()
        .expect("mock updater")
}

fn manifest_endpoint(version: &str, bytes: &'static str, signature: Option<&str>) -> String {
    let (artifact_url, _) = spawn_http_handler(move |_| ("200 OK".into(), bytes.into()));
    let mut platform = json!({"url": format!("{artifact_url}/artifact")});
    if let Some(signature) = signature {
        platform["signature"] = json!(signature.trim());
    }
    let manifest = json!({
        "version": version,
        "notes": "Synthetic release",
        "platforms": {"windows-x86_64": platform}
    })
    .to_string();
    spawn_http_handler(move |_| ("200 OK".into(), manifest.clone())).0
}

#[tokio::test]
async fn real_plugin_download_verifies_signed_fixture_without_installing() {
    let endpoint = manifest_endpoint("0.1.1", ARTIFACT, Some(SIGNATURE));
    let update = updater(&endpoint, "windows-x86_64")
        .check()
        .await
        .unwrap()
        .unwrap();
    let mut received = 0;
    let bytes = update
        .download(|chunk, _| received += chunk, || {})
        .await
        .unwrap();
    assert_eq!(bytes, ARTIFACT.as_bytes());
    assert_eq!(received, bytes.len());
}

#[tokio::test]
async fn real_plugin_rejects_tampered_artifact() {
    let endpoint = manifest_endpoint("0.1.1", "tampered", Some(SIGNATURE));
    let update = updater(&endpoint, "windows-x86_64")
        .check()
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        update.download(|_, _| {}, || {}).await,
        Err(tauri_plugin_updater::Error::Minisign(_))
    ));
}

#[tokio::test]
async fn missing_signature_and_wrong_architecture_are_rejected() {
    let unsigned = manifest_endpoint("0.1.1", ARTIFACT, None);
    assert!(updater(&unsigned, "windows-x86_64").check().await.is_err());
    let signed = manifest_endpoint("0.1.1", ARTIFACT, Some(SIGNATURE));
    assert!(updater(&signed, "windows-aarch64").check().await.is_err());
}

#[tokio::test]
async fn invalid_signature_is_rejected_before_installation() {
    let endpoint = manifest_endpoint("0.1.1", ARTIFACT, Some("not a signature"));
    let update = updater(&endpoint, "windows-x86_64")
        .check()
        .await
        .unwrap()
        .unwrap();
    assert!(update.download(|_, _| {}, || {}).await.is_err());
}

#[tokio::test]
async fn prerelease_and_current_versions_are_not_offered() {
    for version in ["0.1.1-rc.1", "0.1.0", "0.0.9"] {
        let endpoint = manifest_endpoint(version, ARTIFACT, Some(SIGNATURE));
        assert!(
            updater(&endpoint, "windows-x86_64")
                .check()
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn malformed_manifest_and_offline_server_fail_recoverably() {
    let (endpoint, _) = spawn_http_handler(|_| ("200 OK".into(), "not json".into()));
    assert!(updater(&endpoint, "windows-x86_64").check().await.is_err());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    assert!(
        updater(&format!("http://{address}"), "windows-x86_64")
            .check()
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unresponsive_server_obeys_check_timeout() {
    // A listening socket with no responder deterministically holds the HTTP request open.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        updater(&format!("http://{address}"), "windows-x86_64").check(),
    )
    .await;
    assert!(result.expect("request timeout is bounded").is_err());
}
