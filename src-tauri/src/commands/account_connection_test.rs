use std::{sync::Arc, time::Duration};

use tokio::{net::TcpStream, sync::Mutex, time::timeout};

use super::{AccountConnections, CommandError, DesktopOAuthSession};

#[tokio::test]
async fn abandoning_callback_closes_listener_and_allows_immediate_retry() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    connections
        .begin("first".into(), &authentication_lock)
        .await
        .unwrap();
    let mut attempt = connections.take("first").await.unwrap();
    let session = DesktopOAuthSession::start("test-client").await.unwrap();
    let address = session
        .redirect_uri()
        .trim_start_matches("http://")
        .to_owned();
    assert!(authentication_lock.try_lock().is_err());
    let waiting = async move {
        attempt
            .wait_for_authorization(async {
                session
                    .receive_callback()
                    .await
                    .map_err(|error| CommandError::OAuth(error.to_string()))
            })
            .await
    };
    let (result, ()) = timeout(Duration::from_secs(1), async {
        tokio::join!(waiting, connections.cancel("first"))
    })
    .await
    .expect("cancellation releases the callback without waiting five minutes");
    assert!(result.is_err());
    assert!(TcpStream::connect(&address).await.is_err());
    connections
        .begin("second".into(), &authentication_lock)
        .await
        .unwrap();
    connections.cancel("first").await;
    let mut second = connections.take("second").await.unwrap();
    assert_eq!(
        second
            .wait_for_authorization(async { Ok(7) })
            .await
            .unwrap(),
        7
    );
}

#[tokio::test]
async fn cancelling_reservation_before_connect_prevents_late_start() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    connections
        .begin("first".into(), &authentication_lock)
        .await
        .unwrap();
    connections.cancel("first").await;
    assert!(connections.take("first").await.is_err());
    connections
        .begin("second".into(), &authentication_lock)
        .await
        .unwrap();
    connections.cancel("first").await;
    assert!(connections.take("second").await.is_ok());
}

#[tokio::test]
async fn failure_and_timeout_release_attempts() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    for attempt_id in ["failure", "timeout"] {
        connections
            .begin(attempt_id.into(), &authentication_lock)
            .await
            .unwrap();
        {
            let mut attempt = connections.take(attempt_id).await.unwrap();
            let result = attempt
                .wait_for_authorization(async {
                    if attempt_id == "timeout" {
                        let session = DesktopOAuthSession::start("test-client").await.unwrap();
                        let _ =
                            timeout(Duration::from_millis(10), session.receive_callback()).await;
                    }
                    Err::<(), _>(CommandError::OAuth("callback failed".into()))
                })
                .await;
            assert!(result.is_err());
        }
        connections.cancel(attempt_id).await;
        assert!(authentication_lock.try_lock().is_ok());
    }
}

#[tokio::test]
async fn cancellation_waits_for_persistence_without_interrupting_it() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    connections
        .begin("first".into(), &authentication_lock)
        .await
        .unwrap();
    let mut attempt = connections.take("first").await.unwrap();
    attempt
        .wait_for_authorization(async { Ok(()) })
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(10), connections.cancel("first"))
            .await
            .is_err()
    );
    assert!(authentication_lock.try_lock().is_err());
    drop(attempt);
    connections.cancel("first").await;
    assert!(authentication_lock.try_lock().is_ok());
}

#[tokio::test]
async fn cancellation_does_not_unlock_unrelated_authentication() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    let _reauthentication = authentication_lock.lock().await;
    assert!(
        connections
            .begin("first".into(), &authentication_lock)
            .await
            .is_err()
    );
    connections.cancel("first").await;
    assert!(authentication_lock.try_lock().is_err());
}

#[tokio::test]
async fn new_dialog_replaces_unconsumed_reservation_but_not_running_attempt() {
    let connections = AccountConnections::default();
    let authentication_lock = Arc::new(Mutex::new(()));
    connections
        .begin("disappeared-dialog".into(), &authentication_lock)
        .await
        .unwrap();
    connections
        .begin("new-dialog".into(), &authentication_lock)
        .await
        .unwrap();
    assert!(connections.take("disappeared-dialog").await.is_err());
    let _running = connections.take("new-dialog").await.unwrap();
    assert!(
        connections
            .begin("third-dialog".into(), &authentication_lock)
            .await
            .is_err()
    );
    assert!(authentication_lock.try_lock().is_err());
}
