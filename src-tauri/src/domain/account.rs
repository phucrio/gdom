use std::{collections::HashMap, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountError {
    UnsupportedAccountType,
    InvalidLabel,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedAccountType => write!(
                f,
                "only personal Google accounts (@gmail.com / @googlemail.com) are supported"
            ),
            Self::InvalidLabel => write!(f, "account label cannot exceed 100 characters"),
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
pub struct AccountLabel(String);

impl AccountLabel {
    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let trimmed = value.into().trim().to_string();
        if trimmed.is_empty() || trimmed.chars().count() > 100 {
            return Err(AccountError::InvalidLabel);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthStatus {
    #[default]
    Connected,
    TokenRefreshing,
    ReauthRequired,
    Disconnected,
    RemovalPending,
}

impl AuthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connected => "CONNECTED",
            Self::TokenRefreshing => "TOKEN_REFRESHING",
            Self::ReauthRequired => "REAUTH_REQUIRED",
            Self::Disconnected => "DISCONNECTED",
            Self::RemovalPending => "REMOVAL_PENDING",
        }
    }

    pub fn parse_status(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "CONNECTED" => Some(Self::Connected),
            "TOKEN_REFRESHING" => Some(Self::TokenRefreshing),
            "REAUTH_REQUIRED" => Some(Self::ReauthRequired),
            "DISCONNECTED" => Some(Self::Disconnected),
            "REMOVAL_PENDING" => Some(Self::RemovalPending),
            _ => None,
        }
    }
}

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AuthStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_status(s).ok_or(())
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
    label: Option<AccountLabel>,
    auth_status: AuthStatus,
    connected_at: String,
    last_authenticated_at: String,
    updated_at: String,
    removed_at: Option<String>,
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
            label: None,
            auth_status: AuthStatus::Connected,
            connected_at: String::new(),
            last_authenticated_at: String::new(),
            updated_at: String::new(),
            removed_at: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_lifecycle(
        id: AccountId,
        google_permission_id: GooglePermissionId,
        profile: AccountProfile,
        label: Option<AccountLabel>,
        auth_status: AuthStatus,
        connected_at: String,
        last_authenticated_at: String,
        updated_at: String,
        removed_at: Option<String>,
    ) -> Self {
        Self {
            id,
            google_permission_id,
            profile,
            label,
            auth_status,
            connected_at,
            last_authenticated_at,
            updated_at,
            removed_at,
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

    pub fn label(&self) -> Option<&AccountLabel> {
        self.label.as_ref()
    }

    pub fn set_label(&mut self, label: Option<AccountLabel>) {
        self.label = label;
    }

    pub fn auth_status(&self) -> AuthStatus {
        self.auth_status
    }

    pub fn set_auth_status(&mut self, status: AuthStatus) {
        self.auth_status = status;
    }

    pub fn connected_at(&self) -> &str {
        &self.connected_at
    }

    pub fn last_authenticated_at(&self) -> &str {
        &self.last_authenticated_at
    }

    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    pub fn removed_at(&self) -> Option<&str> {
        self.removed_at.as_deref()
    }

    pub fn is_active(&self) -> bool {
        self.removed_at.is_none()
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
        let mut registry = AccountRegistry::default();

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

        assert_eq!(registry.account_count(), 3);
    }

    #[test]
    fn registry_preserves_account_id_when_identity_reconnects() {
        let mut registry = AccountRegistry::default();
        let original_id = registry.connect(ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("permission-a"),
            AccountProfile::new("a@example.com", "Account A"),
        ));

        let reconnected_id = registry.connect(ConnectedAccount::new(
            AccountId::new(99),
            GooglePermissionId::new("permission-a"),
            AccountProfile::new("updated@example.com", "Updated Account A"),
        ));

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

    #[test]
    fn account_label_validates_length() {
        assert!(AccountLabel::new("Work Personal").is_ok());
        let valid_100 = "a".repeat(100);
        assert!(AccountLabel::new(valid_100).is_ok());

        let invalid_101 = "a".repeat(101);
        assert_eq!(
            AccountLabel::new(invalid_101),
            Err(AccountError::InvalidLabel)
        );

        assert_eq!(AccountLabel::new(""), Err(AccountError::InvalidLabel));
        assert_eq!(AccountLabel::new("   "), Err(AccountError::InvalidLabel));
    }

    #[test]
    fn auth_status_serialization_and_parsing() {
        use std::str::FromStr;

        assert_eq!(
            AuthStatus::parse_status("CONNECTED"),
            Some(AuthStatus::Connected)
        );
        assert_eq!(AuthStatus::from_str("CONNECTED"), Ok(AuthStatus::Connected));
        assert_eq!(
            AuthStatus::parse_status("token_refreshing"),
            Some(AuthStatus::TokenRefreshing)
        );
        assert_eq!(
            AuthStatus::parse_status("REAUTH_REQUIRED"),
            Some(AuthStatus::ReauthRequired)
        );
        assert_eq!(
            AuthStatus::parse_status("disconnected"),
            Some(AuthStatus::Disconnected)
        );
        assert_eq!(
            AuthStatus::parse_status("REMOVAL_PENDING"),
            Some(AuthStatus::RemovalPending)
        );
        assert_eq!(AuthStatus::parse_status("unknown"), None);
        assert!(AuthStatus::from_str("unknown").is_err());
    }
}
