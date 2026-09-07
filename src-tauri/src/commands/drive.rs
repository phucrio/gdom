use tauri::State;

use crate::commands::account::parse_account_id;
use crate::commands::drive_dto::{
    DriveFileListDto, ListDriveFilesInput, RenameDriveItemInput, StartTransferOperationInput,
    TrashDriveItemInput,
};
use crate::commands::dto::JobDto;
use crate::commands::error::CommandError;
use crate::state::AppState;

pub(crate) async fn list_drive_files_inner(
    state: &AppState,
    input: ListDriveFilesInput,
) -> Result<DriveFileListDto, CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    let account = state
        .account_store
        .find_by_id(account_id)
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?
        .ok_or_else(|| {
            CommandError::AccountNotFound(format!("Account {} not found", account_id.value()))
        })?;

    let token = state
        .token_provider
        .get_access_token(account_id)
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    let page = state
        .drive_client
        .list_browse_children(
            &token,
            input.folder_id.as_deref(),
            input.page_token.as_deref(),
            input.page_size,
            input.order_by.as_deref(),
        )
        .await
        .map_err(|e| CommandError::DriveApi(e.to_string()))?;

    let active_perm = account.google_permission_id().as_str();
    let items = page
        .files
        .into_iter()
        .map(|file| {
            let is_folder = file.mime_type == "application/vnd.google-apps.folder";
            let is_owner = file
                .owners
                .iter()
                .any(|o| o.permission_id.as_str() == active_perm);
            let owners = file
                .owners
                .into_iter()
                .map(|o| crate::commands::drive_dto::DriveFileOwnerDto {
                    permission_id: o.permission_id.as_str().to_string(),
                    email_address: o.email_address,
                })
                .collect();
            crate::commands::drive_dto::DriveFileItemDto {
                id: file.id,
                name: file.name,
                mime_type: file.mime_type,
                is_folder,
                size: file.quota_bytes_used,
                modified_time: file.modified_time,
                owners,
                web_view_link: file.web_view_link,
                can_transfer_ownership: is_owner,
                is_owner,
                shortcut_target_id: file.shortcut_target_id,
            }
        })
        .collect();

    Ok(DriveFileListDto {
        items,
        next_page_token: page.next_page_token,
    })
}

pub(crate) async fn rename_drive_item_inner(
    state: &AppState,
    input: RenameDriveItemInput,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    let token = state
        .token_provider
        .get_access_token(account_id)
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    state
        .drive_client
        .rename_file(&token, &input.file_id, &input.new_name)
        .await
        .map_err(|e| CommandError::DriveApi(e.to_string()))?;

    Ok(())
}

pub(crate) async fn trash_drive_item_inner(
    state: &AppState,
    input: TrashDriveItemInput,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    let token = state
        .token_provider
        .get_access_token(account_id)
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    state
        .drive_client
        .trash_file(&token, &input.file_id)
        .await
        .map_err(|e| CommandError::DriveApi(e.to_string()))?;

    Ok(())
}

pub(crate) async fn start_transfer_operation_inner(
    state: &AppState,
    input: StartTransferOperationInput,
) -> Result<JobDto, CommandError> {
    let source_id = parse_account_id(&input.source_account_id)?;
    let target_id = parse_account_id(&input.target_account_id)?;

    let job = state
        .job_service
        .start_transfer_operation(source_id, target_id, input.root_file_ids, input.recursive)
        .await
        .map_err(crate::commands::job::map_job_service_error)?;

    crate::commands::job::job_dto_with_scan(state, job).await
}

#[tauri::command]
pub async fn list_drive_files(
    state: State<'_, AppState>,
    input: ListDriveFilesInput,
) -> Result<DriveFileListDto, CommandError> {
    list_drive_files_inner(&state, input).await
}

#[tauri::command]
pub async fn rename_drive_item(
    state: State<'_, AppState>,
    input: RenameDriveItemInput,
) -> Result<(), CommandError> {
    rename_drive_item_inner(&state, input).await
}

#[tauri::command]
pub async fn trash_drive_item(
    state: State<'_, AppState>,
    input: TrashDriveItemInput,
) -> Result<(), CommandError> {
    trash_drive_item_inner(&state, input).await
}

#[tauri::command]
pub async fn start_transfer_operation(
    state: State<'_, AppState>,
    input: StartTransferOperationInput,
) -> Result<JobDto, CommandError> {
    start_transfer_operation_inner(&state, input).await
}
