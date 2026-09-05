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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshotDto {
    pub account_id: String,
    pub email: String,
    pub display_name: String,
    pub permission_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootDto {
    pub id: String,
    pub job_id: String,
    pub root_file_id: String,
    pub root_name: String,
    pub validation_status: String,
    pub created_at: String,
}

impl From<&crate::domain::job::MigrationRoot> for RootDto {
    fn from(root: &crate::domain::job::MigrationRoot) -> Self {
        Self {
            id: root.id.value().to_string(),
            job_id: root.job_id.value().to_string(),
            root_file_id: root.root_file_id.clone(),
            root_name: root.root_name.clone(),
            validation_status: root.validation_status.as_str().to_string(),
            created_at: root.created_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDto {
    pub id: String,
    pub source_account_id: String,
    pub target_account_id: String,
    pub source_snapshot: AccountSnapshotDto,
    pub target_snapshot: AccountSnapshotDto,
    pub status: String,
    pub queue_position: Option<i64>,
    pub canary_size: usize,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_error: Option<String>,
    pub roots: Vec<RootDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan: Option<ScanSummaryDto>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummaryDto {
    pub files: u64,
    pub folders: u64,
    pub skipped: u64,
    pub ineligible: u64,
    pub quota_warning: bool,
}

impl From<&crate::application::PreflightSummary> for ScanSummaryDto {
    fn from(summary: &crate::application::PreflightSummary) -> Self {
        Self {
            files: summary.eligible_files,
            folders: summary.eligible_folders,
            skipped: summary.skipped_total(),
            ineligible: summary.skipped_total(),
            quota_warning: summary.quota_warning,
        }
    }
}

impl From<&crate::domain::job::MigrationJob> for JobDto {
    fn from(job: &crate::domain::job::MigrationJob) -> Self {
        let snapshots = job.snapshots();
        Self {
            id: job.id().value().to_string(),
            source_account_id: job.source_account_id().value().to_string(),
            target_account_id: job.target_account_id().value().to_string(),
            source_snapshot: AccountSnapshotDto {
                account_id: snapshots.source.account_id.value().to_string(),
                email: snapshots.source.email.clone(),
                display_name: snapshots.source.display_name.clone(),
                permission_id: snapshots.source.permission_id.as_str().to_string(),
            },
            target_snapshot: AccountSnapshotDto {
                account_id: snapshots.target.account_id.value().to_string(),
                email: snapshots.target.email.clone(),
                display_name: snapshots.target.display_name.clone(),
                permission_id: snapshots.target.permission_id.as_str().to_string(),
            },
            status: job.status().as_str().to_string(),
            queue_position: job.queue_position(),
            canary_size: job.canary_size(),
            created_at: job.created_at().to_string(),
            started_at: job.started_at().map(ToOwned::to_owned),
            completed_at: job.completed_at().map(ToOwned::to_owned),
            last_error: job.last_error().map(ToOwned::to_owned),
            roots: job.roots().iter().map(RootDto::from).collect(),
            scan: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobItemDto {
    pub id: String,
    pub job_id: String,
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub depth: i64,
    pub original_parent_ids: Vec<String>,
    pub state: String,
    pub quota_bytes_used: Option<i64>,
}

impl From<&crate::domain::MigrationItem> for JobItemDto {
    fn from(item: &crate::domain::MigrationItem) -> Self {
        Self {
            id: item.id.value().to_string(),
            job_id: item.job_id.value().to_string(),
            file_id: item.file_id.clone(),
            name: item.name.clone(),
            mime_type: item.mime_type.clone(),
            depth: item.depth,
            original_parent_ids: item.original_parent_ids.clone(),
            state: item.state.as_str().to_string(),
            quota_bytes_used: item.quota_bytes_used,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobItemsPageDto {
    pub items: Vec<JobItemDto>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunExportDto {
    pub path: String,
    pub eligible_items: u64,
    pub quota_warning: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRootResultDto {
    pub folder_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobInput {
    pub source_account_id: String,
    pub target_account_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDraftJobAccountsInput {
    pub job_id: String,
    pub source_account_id: String,
    pub target_account_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFolderInput {
    pub job_id: String,
    pub input: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRootInput {
    pub job_id: String,
    pub root_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobIdInput {
    pub job_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobsFilter {
    pub status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobItemsInput {
    pub job_id: String,
    pub filter: Option<String>,
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDryRunInput {
    pub job_id: String,
    pub destination: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCanaryInput {
    pub job_id: String,
    pub confirmation: String,
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
