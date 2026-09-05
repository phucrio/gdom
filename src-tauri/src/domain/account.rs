use std::{collections::HashMap, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountError {
    UnsupportedAccountType,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedAccountType => write!(
                f,
                "only personal Google accounts (@gmail.com / @googlemail.com) are supported"
            ),
        }
    }
}

impl Error for AccountError {}

fn is_personal_google_email(email: &str) -> bool {
    let lower = email.trim().to_ascii_lowercase();
    lower.ends_with("@gmail.com") || lower.ends_with("@googlemail.com")
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccountId(pub(super) u128);

impl AccountId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u128 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GooglePermissionId(String);

impl GooglePermissionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountProfile {
    email: String,
    display_name: String,
}

impl AccountProfile {
    pub fn new(email: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            display_name: display_name.into(),
        }
    }

    pub fn new_personal(
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, AccountError> {
        let email = email.into();
        if !is_personal_google_email(&email) {
            return Err(AccountError::UnsupportedAccountType);
        }
        Ok(Self {
            email,
            display_name: display_name.into(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedAccount {
    id: AccountId,
    google_permission_id: GooglePermissionId,
    profile: AccountProfile,
}

impl ConnectedAccount {
    pub const fn new(
        id: AccountId,
        google_permission_id: GooglePermissionId,
        profile: AccountProfile,
    ) -> Self {
        Self {
            id,
            google_permission_id,
            profile,
        }
    }

    pub fn new_personal(
        id: AccountId,
        google_permission_id: GooglePermissionId,
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, AccountError> {
        let profile = AccountProfile::new_personal(email, display_name)?;
        Ok(Self::new(id, google_permission_id, profile))
    }

    pub const fn id(&self) -> AccountId {
        self.id
    }

    pub const fn google_permission_id(&self) -> &GooglePermissionId {
        &self.google_permission_id
    }

    pub fn email(&self) -> &str {
        &self.profile.email
    }

    pub fn display_name(&self) -> &str {
        &self.profile.display_name
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
            AccountProfile::new("a@example.com", "Account A"),
        ));
        registry.connect(ConnectedAccount::new(
            AccountId::new(2),
            GooglePermissionId::new("permission-b"),
            AccountProfile::new("b@example.com", "Account B"),
        ));
        registry.connect(ConnectedAccount::new(
            AccountId::new(3),
            GooglePermissionId::new("permission-c"),
            AccountProfile::new("c@example.com", "Account C"),
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
            AccountProfile::new("a@example.com", "Account A"),
        ));

        // When
        let reconnected_id = registry.connect(ConnectedAccount::new(
            AccountId::new(99),
            GooglePermissionId::new("permission-a"),
            AccountProfile::new("updated@example.com", "Updated Account A"),
        ));

        // Then
        assert_eq!(reconnected_id, original_id);
        assert_eq!(registry.account_count(), 1);
    }

    #[test]
    fn personal_email_validation_accepts_gmail_and_googlemail() {
        assert!(AccountProfile::new_personal("user@gmail.com", "User").is_ok());
        assert!(AccountProfile::new_personal("user@googlemail.com", "User").is_ok());
        assert!(AccountProfile::new_personal("User.Name+Tag@GMAIL.COM", "User").is_ok());
        assert!(AccountProfile::new_personal("User@GoogleMail.Com", "User").is_ok());
    }

    #[test]
    fn personal_email_validation_rejects_workspace_and_other_domains() {
        assert_eq!(
            AccountProfile::new_personal("admin@company.com", "Workspace User"),
            Err(AccountError::UnsupportedAccountType)
        );
        assert_eq!(
            AccountProfile::new_personal("user@notgmail.com", "Impostor"),
            Err(AccountError::UnsupportedAccountType)
        );
        assert_eq!(
            AccountProfile::new_personal("gmail.com", "Malformed"),
            Err(AccountError::UnsupportedAccountType)
        );
        assert_eq!(
            AccountProfile::new_personal("", "Empty"),
            Err(AccountError::UnsupportedAccountType)
        );
    }
}
