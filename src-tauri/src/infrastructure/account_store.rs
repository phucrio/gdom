use std::num::ParseIntError;
use std::path::Path;
use std::{error::Error, fmt};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::{
    application::{AccountStorePort, AccountStorePortError},
    domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
};

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
    pub(crate) async fn open_in_memory() -> Result<Self, AccountStoreError> {
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

    #[cfg(test)]
    pub(crate) async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn list_all(&self) -> Result<Vec<ConnectedAccount>, AccountStoreError> {
        let rows = sqlx::query_as::<_, StoredAccountRow>(
            "SELECT id, google_permission_id, email, display_name FROM accounts ORDER BY email ASC",
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

    pub async fn save_oauth_config(
        &self,
        client_id: &str,
        client_secret: Option<&str>,
    ) -> Result<(), AccountStoreError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('oauth.client_id', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(client_id)
        .execute(&mut *tx)
        .await?;

        if let Some(secret) = client_secret {
            sqlx::query(
                "INSERT INTO app_settings (key, value) VALUES ('oauth.client_secret', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )
            .bind(secret)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query("DELETE FROM app_settings WHERE key = 'oauth.client_secret'")
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
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

impl AccountStorePort for SqliteAccountStore {
    async fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        self.find_by_permission_id(permission_id)
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

    async fn remove(&self, account_id: AccountId) -> Result<(), AccountStorePortError> {
        self.remove(account_id)
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
