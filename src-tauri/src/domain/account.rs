use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccountId(pub(super) u128);

impl AccountId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct GooglePermissionId(String);

impl GooglePermissionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConnectedAccount {
    id: AccountId,
    google_permission_id: GooglePermissionId,
}

impl ConnectedAccount {
    pub const fn new(id: AccountId, google_permission_id: GooglePermissionId) -> Self {
        Self {
            id,
            google_permission_id,
        }
    }
}

#[derive(Debug, Default)]
pub struct AccountRegistry {
    accounts: HashMap<GooglePermissionId, AccountId>,
}

impl AccountRegistry {
    pub fn connect(&mut self, account: ConnectedAccount) -> AccountId {
        *self
            .accounts
            .entry(account.google_permission_id)
            .or_insert(account.id)
    }

    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_accepts_more_than_two_accounts() {
        // Given
        let mut registry = AccountRegistry::default();

        // When
        registry.connect(ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("permission-a"),
        ));
        registry.connect(ConnectedAccount::new(
            AccountId::new(2),
            GooglePermissionId::new("permission-b"),
        ));
        registry.connect(ConnectedAccount::new(
            AccountId::new(3),
            GooglePermissionId::new("permission-c"),
        ));

        // Then
        assert_eq!(registry.account_count(), 3);
    }

    #[test]
    fn registry_preserves_account_id_when_identity_reconnects() {
        // Given
        let mut registry = AccountRegistry::default();
        let original_id = registry.connect(ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("permission-a"),
        ));

        // When
        let reconnected_id = registry.connect(ConnectedAccount::new(
            AccountId::new(99),
            GooglePermissionId::new("permission-a"),
        ));

        // Then
        assert_eq!(reconnected_id, original_id);
        assert_eq!(registry.account_count(), 1);
    }
}
