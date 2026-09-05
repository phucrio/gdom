use crate::domain::{AccountLabel, AuthStatus, ConnectedAccount};

/// Serializable account representation for the frontend.
///
/// Deliberately omits tokens and credentials — only identity, status,
/// and lifecycle metadata cross the IPC boundary.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub google_permission_id: String,
    pub email: String,
    pub display_name: String,
    pub label: Option<String>,
    pub auth_status: AuthStatus,
    pub connected_at: String,
    pub last_authenticated_at: String,
    pub updated_at: String,
    pub removed_at: Option<String>,
}

impl From<&ConnectedAccount> for AccountDto {
    fn from(account: &ConnectedAccount) -> Self {
        Self {
            id: account.id().value().to_string(),
            google_permission_id: account.google_permission_id().as_str().to_owned(),
            email: account.email().to_owned(),
            display_name: account.display_name().to_owned(),
            label: account
                .label()
                .map(AccountLabel::as_str)
                .map(ToOwned::to_owned),
            auth_status: account.auth_status(),
            connected_at: account.connected_at().to_owned(),
            last_authenticated_at: account.last_authenticated_at().to_owned(),
            updated_at: account.updated_at().to_owned(),
            removed_at: account.removed_at().map(ToOwned::to_owned),
        }
    }
}

impl From<ConnectedAccount> for AccountDto {
    fn from(account: ConnectedAccount) -> Self {
        Self::from(&account)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountLabelInput {
    pub account_id: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdInput {
    pub account_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountDataInput {
    pub account_id: String,
    pub confirmation: bool,
}

#[derive(Clone, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureOAuthInput {
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl std::fmt::Debug for ConfigureOAuthInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigureOAuthInput")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// Read-only snapshot of whether OAuth is configured.
///
/// Exposes only the client ID (non-secret); the client secret is never
/// sent to the frontend.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfigDto {
    pub is_configured: bool,
    pub client_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId};

    fn sample_account() -> ConnectedAccount {
        ConnectedAccount::new(
            AccountId::new(42),
            GooglePermissionId::new("perm-abc123"),
            AccountProfile::new("user@gmail.com", "Test User"),
        )
    }

    #[test]
    fn account_dto_from_ref_maps_all_fields() {
        let account = sample_account();
        let dto = AccountDto::from(&account);

        assert_eq!(dto.id, "42");
        assert_eq!(dto.google_permission_id, "perm-abc123");
        assert_eq!(dto.email, "user@gmail.com");
        assert_eq!(dto.display_name, "Test User");
        assert_eq!(dto.auth_status, AuthStatus::Connected);
        assert_eq!(dto.label, None);
    }

    #[test]
    fn account_dto_from_owned_matches_ref() {
        let account = sample_account();
        let from_ref = AccountDto::from(&account);
        let from_owned = AccountDto::from(account);

        assert_eq!(from_ref, from_owned);
    }

    #[test]
    fn account_dto_serializes_as_camel_case() {
        let dto = AccountDto {
            id: "1".into(),
            google_permission_id: "perm-x".into(),
            email: "a@b.com".into(),
            display_name: "A B".into(),
            label: Some("My Label".into()),
            auth_status: AuthStatus::Connected,
            connected_at: "2026-09-05T00:00:00Z".into(),
            last_authenticated_at: "2026-09-05T00:00:00Z".into(),
            updated_at: "2026-09-05T00:00:00Z".into(),
            removed_at: None,
        };
        let json = serde_json::to_value(&dto).expect("serializes");

        assert_eq!(json["id"], "1");
        assert_eq!(json["googlePermissionId"], "perm-x");
        assert_eq!(json["email"], "a@b.com");
        assert_eq!(json["displayName"], "A B");
        assert_eq!(json["label"], "My Label");
        assert_eq!(json["authStatus"], "CONNECTED");
    }

    #[test]
    fn account_dto_roundtrips_through_json() {
        let dto = AccountDto {
            id: "99".into(),
            google_permission_id: "perm-z".into(),
            email: "z@example.com".into(),
            display_name: "Z".into(),
            label: None,
            auth_status: AuthStatus::TokenRefreshing,
            connected_at: "2026-09-05T00:00:00Z".into(),
            last_authenticated_at: "2026-09-05T00:00:00Z".into(),
            updated_at: "2026-09-05T00:00:00Z".into(),
            removed_at: None,
        };
        let json = serde_json::to_string(&dto).expect("serializes");
        let restored: AccountDto = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(dto, restored);
    }

    #[test]
    fn oauth_config_dto_serializes_as_camel_case() {
        let dto = OAuthConfigDto {
            is_configured: true,
            client_id: Some("client-123".into()),
        };
        let json = serde_json::to_value(&dto).expect("serializes");

        assert_eq!(json["isConfigured"], true);
        assert_eq!(json["clientId"], "client-123");
    }

    #[test]
    fn oauth_config_dto_serializes_none_client_id_as_null() {
        let dto = OAuthConfigDto {
            is_configured: false,
            client_id: None,
        };
        let json = serde_json::to_value(&dto).expect("serializes");

        assert_eq!(json["isConfigured"], false);
        assert!(json["clientId"].is_null());
    }

    #[test]
    fn configure_oauth_input_deserializes_with_secret() {
        let json = r#"{"clientId":"id-1","clientSecret":"secret-1"}"#;
        let input: ConfigureOAuthInput = serde_json::from_str(json).expect("deserializes");

        assert_eq!(input.client_id, "id-1");
        assert_eq!(input.client_secret.as_deref(), Some("secret-1"));
    }

    #[test]
    fn configure_oauth_input_deserializes_without_secret() {
        let json = r#"{"clientId":"id-2"}"#;
        let input: ConfigureOAuthInput = serde_json::from_str(json).expect("deserializes");

        assert_eq!(input.client_id, "id-2");
        assert_eq!(input.client_secret, None);
    }
}
