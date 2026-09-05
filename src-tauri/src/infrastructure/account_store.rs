use std::num::ParseIntError;
use std::path::Path;
use std::{error::Error, fmt};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::{
    application::{AccountStorePort, AccountStorePortError},
    domain::{
        AccountError, AccountId, AccountLabel, AccountProfile, AuthStatus, ConnectedAccount,
        GooglePermissionId,
    },
};

#[derive(sqlx::FromRow)]
struct StoredAccountRow {
    id: String,
    google_permission_id: String,
    email: String,
    display_name: String,
    label: Option<String>,
    auth_status: String,
    connected_at: String,
    last_authenticated_at: String,
    updated_at: String,
    removed_at: Option<String>,
}

pub struct SqliteAccountStore {
    pool: SqlitePool,
}

#[derive(Debug)]
pub enum AccountStoreError {
    Database(sqlx::Error),
    InvalidAccountId(ParseIntError),
    AccountDomain(AccountError),
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
            Self::AccountDomain(error) => {
                write!(formatter, "invalid account data in database: {error}")
            }
        }
    }
}

impl Error for AccountStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidAccountId(error) => Some(error),
            Self::AccountDomain(error) => Some(error),
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

impl From<AccountError> for AccountStoreError {
    fn from(error: AccountError) -> Self {
        Self::AccountDomain(error)
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
    pub(crate) async fn open_in_memory() -> Result<Self, AccountStoreError> {
        Self::from_options(
            SqliteConnectOptions::new()
                .in_memory(true)
                .foreign_keys(true),
        )
        .await
    }

    async fn from_options(options: SqliteConnectOptions) -> Result<Self, AccountStoreError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            ) STRICT;",
        )
        .execute(&pool)
        .await?;

        let row: Option<(i64,)> =
            sqlx::query_as("SELECT version FROM _schema_migrations WHERE version = 1")
                .fetch_optional(&pool)
                .await?;
        if row.is_none() {
            let mut tx = pool.begin().await?;
            sqlx::raw_sql(include_str!("../../migrations/001_accounts.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO _schema_migrations (version, applied_at) VALUES (1, datetime('now'))",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }

        let row: Option<(i64,)> =
            sqlx::query_as("SELECT version FROM _schema_migrations WHERE version = 2")
                .fetch_optional(&pool)
                .await?;
        if row.is_none() {
            let mut tx = pool.begin().await?;
            sqlx::raw_sql(include_str!("../../migrations/002_account_lifecycle.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO _schema_migrations (version, applied_at) VALUES (2, datetime('now'))",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }

        Ok(Self { pool })
    }

    pub async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStoreError> {
        let label_str = account.label().map(AccountLabel::as_str);
        let stored = sqlx::query_as::<_, StoredAccountRow>(
            "INSERT INTO accounts (id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), NULL)
             ON CONFLICT (google_permission_id) DO UPDATE SET
                 email = excluded.email,
                 display_name = excluded.display_name,
                 auth_status = 'CONNECTED',
                 last_authenticated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 removed_at = NULL
             RETURNING id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at",
        )
        .bind(account.id().value().to_string())
        .bind(account.google_permission_id().as_str())
        .bind(account.email())
        .bind(account.display_name())
        .bind(label_str)
        .bind(account.auth_status().as_str())
        .fetch_one(&self.pool)
        .await?;

        parse_account(stored)
    }

    pub async fn account_count(&self) -> Result<i64, AccountStoreError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE removed_at IS NULL")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at
             FROM accounts
             WHERE google_permission_id = ?1 AND removed_at IS NULL",
        )
        .bind(permission_id.as_str())
        .fetch_optional(&self.pool)
        .await?
        .map(parse_account)
        .transpose()
    }

    pub async fn find_any_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at
             FROM accounts
             WHERE google_permission_id = ?1",
        )
        .bind(permission_id.as_str())
        .fetch_optional(&self.pool)
        .await?
        .map(parse_account)
        .transpose()
    }

    pub async fn find_by_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at
             FROM accounts
             WHERE id = ?1 AND removed_at IS NULL",
        )
        .bind(account_id.value().to_string())
        .fetch_optional(&self.pool)
        .await?
        .map(parse_account)
        .transpose()
    }

    pub async fn update_auth_status(
        &self,
        account_id: AccountId,
        status: AuthStatus,
    ) -> Result<(), AccountStoreError> {
        sqlx::query(
            "UPDATE accounts SET auth_status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        )
        .bind(status.as_str())
        .bind(account_id.value().to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_label(
        &self,
        account_id: AccountId,
        label: Option<&AccountLabel>,
    ) -> Result<(), AccountStoreError> {
        let label_str = label.map(AccountLabel::as_str);
        sqlx::query(
            "UPDATE accounts SET label = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        )
        .bind(label_str)
        .bind(account_id.value().to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_last_authenticated(
        &self,
        account_id: AccountId,
    ) -> Result<(), AccountStoreError> {
        sqlx::query(
            "UPDATE accounts SET auth_status = 'CONNECTED', last_authenticated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        )
        .bind(account_id.value().to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn soft_remove(&self, account_id: AccountId) -> Result<(), AccountStoreError> {
        sqlx::query(
            "UPDATE accounts SET auth_status = 'DISCONNECTED', removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        )
        .bind(account_id.value().to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn hard_delete(&self, account_id: AccountId) -> Result<(), AccountStoreError> {
        sqlx::query("DELETE FROM accounts WHERE id = ?1")
            .bind(account_id.value().to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove(&self, account_id: AccountId) -> Result<(), AccountStoreError> {
        self.soft_remove(account_id).await
    }

    #[cfg(test)]
    pub(crate) async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn list_all(&self) -> Result<Vec<ConnectedAccount>, AccountStoreError> {
        let rows = sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name, label, auth_status, connected_at, last_authenticated_at, updated_at, removed_at
             FROM accounts
             WHERE removed_at IS NULL
             ORDER BY email ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(parse_account).collect()
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, AccountStoreError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_settings WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(val,)| val))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), AccountStoreError> {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> Result<(), AccountStoreError> {
        sqlx::query("DELETE FROM app_settings WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn save_oauth_client_id(&self, client_id: &str) -> Result<(), AccountStoreError> {
        self.set_setting("oauth.client_id", client_id).await
    }
}

fn parse_account(stored: StoredAccountRow) -> Result<ConnectedAccount, AccountStoreError> {
    let label = match stored.label {
        Some(l) if !l.trim().is_empty() => Some(AccountLabel::new(l)?),
        _ => None,
    };
    let auth_status =
        AuthStatus::parse_status(&stored.auth_status).unwrap_or(AuthStatus::Connected);

    Ok(ConnectedAccount::with_lifecycle(
        AccountId::new(stored.id.parse::<u128>()?),
        GooglePermissionId::new(stored.google_permission_id),
        AccountProfile::new(stored.email, stored.display_name),
        label,
        auth_status,
        stored.connected_at,
        stored.last_authenticated_at,
        stored.updated_at,
        stored.removed_at,
    ))
}

impl AccountStorePort for SqliteAccountStore {
    async fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        self.find_by_permission_id(permission_id)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn find_by_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        self.find_by_id(account_id)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStorePortError> {
        self.connect(account)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn update_auth_status(
        &self,
        account_id: AccountId,
        status: AuthStatus,
    ) -> Result<(), AccountStorePortError> {
        self.update_auth_status(account_id, status)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn update_label(
        &self,
        account_id: AccountId,
        label: Option<&AccountLabel>,
    ) -> Result<(), AccountStorePortError> {
        self.update_label(account_id, label)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn mark_last_authenticated(
        &self,
        account_id: AccountId,
    ) -> Result<(), AccountStorePortError> {
        self.mark_last_authenticated(account_id)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn remove(&self, account_id: AccountId) -> Result<(), AccountStorePortError> {
        self.remove(account_id)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn hard_delete(&self, account_id: AccountId) -> Result<(), AccountStorePortError> {
        self.hard_delete(account_id)
            .await
            .map_err(AccountStorePortError::from)
    }

    async fn list_all(&self) -> Result<Vec<ConnectedAccount>, AccountStorePortError> {
        self.list_all().await.map_err(AccountStorePortError::from)
    }
}

impl From<AccountStoreError> for AccountStorePortError {
    fn from(error: AccountStoreError) -> Self {
        Self::Storage(error.to_string())
    }
}
