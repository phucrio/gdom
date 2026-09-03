use std::path::Path;
use std::{error::Error, fmt};

use rusqlite::{Connection, OptionalExtension as _, Row, params, types::Type};

use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

pub struct SqliteAccountStore {
    connection: Connection,
}

#[derive(Debug)]
pub struct AccountStoreError(rusqlite::Error);

impl fmt::Display for AccountStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "account database operation failed: {}", self.0)
    }
}

impl Error for AccountStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl From<rusqlite::Error> for AccountStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self(error)
    }
}

impl SqliteAccountStore {
    pub fn open(path: &Path) -> Result<Self, AccountStoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, AccountStoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, AccountStoreError> {
        connection.execute_batch(include_str!("migrations/001_accounts.sql"))?;
        Ok(Self { connection })
    }

    pub fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStoreError> {
        Ok(self.connection.query_row(
            "INSERT INTO accounts (id, google_permission_id, email, display_name)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (google_permission_id) DO UPDATE SET
                 email = excluded.email,
                 display_name = excluded.display_name
             RETURNING id, google_permission_id, email, display_name",
            params![
                account.id().value().to_string(),
                account.google_permission_id().as_str(),
                account.email(),
                account.display_name(),
            ],
            map_account,
        )?)
    }

    pub fn account_count(&self) -> Result<i64, AccountStoreError> {
        Ok(self
            .connection
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?)
    }

    pub fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id, google_permission_id, email, display_name
                 FROM accounts
                 WHERE google_permission_id = ?1",
                [permission_id.as_str()],
                map_account,
            )
            .optional()?)
    }
}

fn map_account(row: &Row<'_>) -> rusqlite::Result<ConnectedAccount> {
    let raw_id = row.get::<_, String>(0)?;
    let id = raw_id
        .parse::<u128>()
        .map(AccountId::new)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
        })?;

    Ok(ConnectedAccount::new(
        id,
        GooglePermissionId::new(row.get::<_, String>(1)?),
        AccountProfile::new(row.get::<_, String>(2)?, row.get::<_, String>(3)?),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

    use super::SqliteAccountStore;

    fn account(id: u128, permission_id: &str, profile: AccountProfile) -> ConnectedAccount {
        ConnectedAccount::new(
            AccountId::new(id),
            GooglePermissionId::new(permission_id),
            profile,
        )
    }

    #[test]
    fn store_accepts_more_than_two_accounts() {
        // Given
        let store = SqliteAccountStore::open_in_memory().expect("in-memory database opens");

        // When
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("a@example.com", "Account A"),
            ))
            .expect("first account persists");
        store
            .connect(&account(
                2,
                "permission-b",
                AccountProfile::new("b@example.com", "Account B"),
            ))
            .expect("second account persists");
        store
            .connect(&account(
                3,
                "permission-c",
                AccountProfile::new("c@example.com", "Account C"),
            ))
            .expect("third account persists");

        // Then
        assert_eq!(store.account_count().expect("account count loads"), 3);
    }

    #[test]
    fn reconnect_preserves_id_and_updates_profile() {
        // Given
        let store = SqliteAccountStore::open_in_memory().expect("in-memory database opens");
        store
            .connect(&account(
                1,
                "permission-a",
                AccountProfile::new("old@example.com", "Old Name"),
            ))
            .expect("original account persists");

        // When
        let reconnected = store
            .connect(&account(
                99,
                "permission-a",
                AccountProfile::new("new@example.com", "New Name"),
            ))
            .expect("account reconnects");

        // Then
        assert_eq!(reconnected.id(), AccountId::new(1));
        assert_eq!(reconnected.email(), "new@example.com");
        assert_eq!(reconnected.display_name(), "New Name");
        assert_eq!(store.account_count().expect("account count loads"), 1);
        assert_eq!(
            store
                .find_by_permission_id(&GooglePermissionId::new("permission-a"))
                .expect("account lookup succeeds"),
            Some(reconnected)
        );
    }

    #[test]
    fn account_survives_database_reopen() {
        // Given
        static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gdom-account-store-{}-{sequence}.sqlite",
            std::process::id()
        ));
        {
            let store = SqliteAccountStore::open(&path).expect("database opens");
            store
                .connect(&account(
                    1,
                    "permission-a",
                    AccountProfile::new("a@example.com", "Account A"),
                ))
                .expect("account persists");
        }

        // When
        let reopened = SqliteAccountStore::open(&path).expect("database reopens");

        // Then
        assert_eq!(reopened.account_count().expect("account count loads"), 1);
        drop(reopened);
        std::fs::remove_file(path).expect("test database is removed");
    }
}
