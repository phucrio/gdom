use serde::{Deserialize, Serialize};

use crate::commands::dto::JobDto;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageQuotaDto {
    pub usage_bytes: u64,
    pub limit_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFileOwnerDto {
    pub permission_id: String,
    pub email_address: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFileItemDto {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub is_folder: bool,
    pub folder_id: Option<String>,
    pub folder_resource_key: Option<String>,
    pub resource_key: Option<String>,
    pub size: Option<i64>,
    pub modified_time: Option<String>,
    pub owners: Vec<DriveFileOwnerDto>,
    pub web_view_link: Option<String>,
    pub can_transfer_ownership: bool,
    pub is_owner: bool,
    pub shortcut_target_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFileListDto {
    pub items: Vec<DriveFileItemDto>,
    pub next_page_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDriveFilesInput {
    pub account_id: String,
    pub folder_id: Option<String>,
    pub folder_resource_key: Option<String>,
    pub page_token: Option<String>,
    pub page_size: Option<u32>,
    pub order_by: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenDriveItemInput {
    pub account_id: String,
    pub file_id: String,
    pub resource_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameDriveItemInput {
    pub account_id: String,
    pub file_id: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashDriveItemInput {
    pub account_id: String,
    pub file_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTransferOperationInput {
    pub source_account_id: String,
    pub target_account_id: String,
    pub root_file_ids: Vec<String>,
    pub recursive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferOperationResponseDto {
    pub job: JobDto,
}
