use std::num::ParseIntError;
use std::path::Path;
use std::{error::Error, fmt};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

#[derive(sqlx::FromRow)]
struct StoredAccountRow {
    id: String,
    google_permission_id: String,
    email: String,
    display_name: String,
}

pub struct SqliteAccountStore {
    pool: SqlitePool,
}

#[derive(Debug)]
pub enum AccountStoreError {
    Database(sqlx::Error),
    InvalidAccountId(ParseIntError),
}

impl fmt::Display for AccountStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => {
                write!(formatter, "account database operation failed: {error}")
            }
            Self::InvalidAccountId(error) => {
                write!(formatter, "stored account ID is invalid: {error}")
            }
        }
    }
}

impl Error for AccountStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidAccountId(error) => Some(error),
        }
    }
}

impl From<sqlx::Error> for AccountStoreError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<ParseIntError> for AccountStoreError {
    fn from(error: ParseIntError) -> Self {
        Self::InvalidAccountId(error)
    }
}

impl SqliteAccountStore {
    pub async fn open(path: &Path) -> Result<Self, AccountStoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        Self::from_options(options).await
    }

    #[cfg(test)]
    async fn open_in_memory() -> Result<Self, AccountStoreError> {
        Self::from_options(
            SqliteConnectOptions::new()
                .in_memory(true)
                .foreign_keys(true),
        )
        .await
    }

    async fn from_options(options: SqliteConnectOptions) -> Result<Self, AccountStoreError> {
        // ponytail: one connection is enough for the local MVP; raise only if concurrent scans contend.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::raw_sql(include_str!("../../migrations/001_accounts.sql"))
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStoreError> {
        let stored = sqlx::query_as::<_, StoredAccountRow>(
            "INSERT INTO accounts (id, google_permission_id, email, display_name)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (google_permission_id) DO UPDATE SET
                 email = excluded.email,
                 display_name = excluded.display_name
             RETURNING id, google_permission_id, email, display_name",
        )
        .bind(account.id().value().to_string())
        .bind(account.google_permission_id().as_str())
        .bind(account.email())
        .bind(account.display_name())
        .fetch_one(&self.pool)
        .await?;

        parse_account(stored)
    }

    pub async fn account_count(&self) -> Result<i64, AccountStoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name
             FROM accounts
             WHERE google_permission_id = ?1",
        )
        .bind(permission_id.as_str())
        .fetch_optional(&self.pool)
        .await?
        .map(parse_account)
        .transpose()
    }

    pub async fn remove(&self, account_id: AccountId) -> Result<(), AccountStoreError> {
        sqlx::query("DELETE FROM accounts WHERE id = ?1")
            .bind(account_id.value().to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn parse_account(stored: StoredAccountRow) -> Result<ConnectedAccount, AccountStoreError> {
    Ok(ConnectedAccount::new(
        AccountId::new(stored.id.parse::<u128>()?),
        GooglePermissionId::new(stored.google_permission_id),
        AccountProfile::new(stored.email, stored.display_name),
    ))
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use tokio::time::timeout;

    use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

    use super::SqliteAccountStore;

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
            store.pool.close().await;

            // When
            let reopened = SqliteAccountStore::open(&path)
                .await
                .expect("database reopens");

            // Then
            assert_eq!(
                reopened.account_count().await.expect("account count loads"),
                1
            );
            reopened.pool.close().await;
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
}
