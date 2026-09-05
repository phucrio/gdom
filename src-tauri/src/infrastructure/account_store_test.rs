use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::time::timeout;

use super::account_store::SqliteAccountStore;
use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

async fn complete<F: Future<Output = ()>>(scenario: F) {
    timeout(Duration::from_secs(5), scenario)
        .await
        .expect("database scenario completes before timeout");
}

fn account(id: u128, permission_id: &str, profile: AccountProfile) -> ConnectedAccount {
    ConnectedAccount::new(
        AccountId::new(id),
        GooglePermissionId::new(permission_id),
        profile,
    )
}

#[tokio::test]
async fn store_accepts_more_than_two_accounts() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");

        // When
        for account in [
            account(
                1,
                "permission-a",
                AccountProfile::new("a@example.com", "Account A"),
            ),
            account(
                2,
                "permission-b",
                AccountProfile::new("b@example.com", "Account B"),
            ),
            account(
                3,
                "permission-c",
                AccountProfile::new("c@example.com", "Account C"),
            ),
        ] {
            store.connect(&account).await.expect("account persists");
        }

        // Then
        assert_eq!(store.account_count().await.expect("account count loads"), 3);
    })
    .await;
}

#[tokio::test]
async fn reconnect_preserves_id_and_updates_profile() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("old@example.com", "Old Name"),
            ))
            .await
            .expect("original account persists");

        // When
        let reconnected = store
            .connect(&account(
                99,
                "permission-a",
                AccountProfile::new("new@example.com", "New Name"),
            ))
            .await
            .expect("account reconnects");

        // Then
        assert_eq!(reconnected.id(), AccountId::new(1));
        assert_eq!(reconnected.email(), "new@example.com");
        assert_eq!(reconnected.display_name(), "New Name");
        assert_eq!(store.account_count().await.expect("account count loads"), 1);
        assert_eq!(
            store
                .find_by_permission_id(&GooglePermissionId::new("permission-a"))
                .await
                .expect("account lookup succeeds"),
            Some(reconnected)
        );
    })
    .await;
}

#[tokio::test]
async fn account_survives_database_reopen() {
    complete(async {
        // Given
        static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gdom-account-store-{}-{sequence}.sqlite",
            std::process::id()
        ));
        let store = SqliteAccountStore::open(&path)
            .await
            .expect("database opens");
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("a@example.com", "Account A"),
            ))
            .await
            .expect("account persists");
        store.close().await;

        // When
        let reopened = SqliteAccountStore::open(&path)
            .await
            .expect("database reopens");

        // Then
        assert_eq!(
            reopened.account_count().await.expect("account count loads"),
            1
        );
        reopened.close().await;
        std::fs::remove_file(path).expect("test database is removed");
    })
    .await;
}

#[tokio::test]
async fn remove_deletes_specified_account() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("a@example.com", "Account A"),
            ))
            .await
            .expect("account persists");
        assert_eq!(store.account_count().await.expect("account count"), 1);

        // When
        store
            .remove(AccountId::new(1))
            .await
            .expect("account removal succeeds");

        // Then
        assert_eq!(store.account_count().await.expect("account count"), 0);
        assert_eq!(
            store
                .find_by_permission_id(&GooglePermissionId::new("permission-a"))
                .await
                .expect("lookup succeeds"),
            None
        );
    })
    .await;
}

#[tokio::test]
async fn list_all_returns_accounts_in_order() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");

        store
            .connect(&account(
                3,
                "permission-c",
                AccountProfile::new("charlie@example.com", "Charlie"),
            ))
            .await
            .expect("account persists");
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("alice@example.com", "Alice"),
            ))
            .await
            .expect("account persists");
        store
            .connect(&account(
                2,
                "permission-b",
                AccountProfile::new("bob@example.com", "Bob"),
            ))
            .await
            .expect("account persists");

        // When
        let accounts = store.list_all().await.expect("list_all succeeds");

        // Then
        assert_eq!(accounts.len(), 3);
        assert_eq!(accounts[0].email(), "alice@example.com");
        assert_eq!(accounts[1].email(), "bob@example.com");
        assert_eq!(accounts[2].email(), "charlie@example.com");
    })
    .await;
}

#[tokio::test]
async fn app_settings_persist_and_update() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");

        assert_eq!(store.get_setting("test_key").await.unwrap(), None);

        // When
        store
            .set_setting("test_key", "value_1")
            .await
            .expect("set_setting succeeds");

        // Then
        assert_eq!(
            store.get_setting("test_key").await.unwrap(),
            Some("value_1".to_owned())
        );

        // When
        store
            .set_setting("test_key", "value_2")
            .await
            .expect("update succeeds");

        // Then
        assert_eq!(
            store.get_setting("test_key").await.unwrap(),
            Some("value_2".to_owned())
        );

        // When
        store
            .delete_setting("test_key")
            .await
            .expect("delete succeeds");

        // Then
        assert_eq!(store.get_setting("test_key").await.unwrap(), None);
    })
    .await;
}

#[tokio::test]
async fn save_oauth_config_atomically_persists_and_deletes_secret() {
    complete(async {
        // Given
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory database opens");

        // When
        store
            .save_oauth_config("client-123", Some("secret-456"))
            .await
            .expect("save succeeds");

        // Then
        assert_eq!(
            store.get_setting("oauth.client_id").await.unwrap(),
            Some("client-123".to_owned())
        );
        assert_eq!(
            store.get_setting("oauth.client_secret").await.unwrap(),
            Some("secret-456".to_owned())
        );

        // When
        store
            .save_oauth_config("client-789", None)
            .await
            .expect("save without secret succeeds");

        // Then
        assert_eq!(
            store.get_setting("oauth.client_id").await.unwrap(),
            Some("client-789".to_owned())
        );
        assert_eq!(
            store.get_setting("oauth.client_secret").await.unwrap(),
            None
        );
    })
    .await;
}
