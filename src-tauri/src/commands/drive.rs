use crate::application::drive_tree::{FOLDER_MIME_TYPE, SHORTCUT_MIME_TYPE};
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
    let page = state
        .drive_browser
        .list_files(
            account_id,
            crate::application::drive_browser::BrowseFolderRequest {
                folder_id: input.folder_id,
                page_token: input.page_token,
                page_size: input.page_size,
                order_by: input.order_by,
            },
        )
        .await
        .map_err(map_browser_error)?;

    let items = page
        .items
        .into_iter()
        .map(|item| {
            let file = item.file;
            let is_owner = item.is_owner;
            let is_folder = file.mime_type == FOLDER_MIME_TYPE;
            let folder_id = if is_folder {
                Some(file.id.clone())
            } else if file.mime_type == SHORTCUT_MIME_TYPE
                && file.shortcut_target_mime_type.as_deref() == Some(FOLDER_MIME_TYPE)
            {
                file.shortcut_target_id.clone()
            } else {
                None
            };
            let owners = file
                .owners
                .into_iter()
                .map(|o| crate::commands::drive_dto::DriveFileOwnerDto {
                    permission_id: o.permission_id.as_str().to_string(),
                    email_address: o.email_address,
                    avatar_url: o.avatar_url,
                })
                .collect();
            crate::commands::drive_dto::DriveFileItemDto {
                id: file.id,
                name: file.name,
                mime_type: file.mime_type,
                is_folder,
                folder_id,
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
    state
        .drive_browser
        .rename_file(
            account_id,
            crate::application::drive_browser::RenameFileRequest {
                file_id: input.file_id,
                new_name: input.new_name,
            },
        )
        .await
        .map_err(map_browser_error)?;

    Ok(())
}

pub(crate) async fn trash_drive_item_inner(
    state: &AppState,
    input: TrashDriveItemInput,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    state
        .drive_browser
        .trash_file(account_id, &input.file_id)
        .await
        .map_err(map_browser_error)?;

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

fn map_browser_error(error: crate::application::drive_browser::DriveBrowserError) -> CommandError {
    use crate::application::drive_browser::DriveBrowserError;
    match error {
        DriveBrowserError::AccountNotFound => {
            CommandError::AccountNotFound("account not found".into())
        }
        DriveBrowserError::Database(error) => CommandError::Database(error.to_string()),
        DriveBrowserError::Authorization(error) => CommandError::OAuth(error.to_string()),
        DriveBrowserError::Drive(error) => CommandError::DriveApi(error.to_string()),
        DriveBrowserError::InvalidInput(message) => CommandError::DriveApi(message.into()),
    }
}

#[tauri::command]
pub fn open_drive_item(file_id: String) -> Result<(), CommandError> {
    let url = drive_item_url(&file_id)?;
    open::that(url.as_str()).map_err(|_| {
        CommandError::BrowserLaunchFailed("could not open Google Drive in the browser".into())
    })
}

fn drive_item_url(file_id: &str) -> Result<url::Url, CommandError> {
    if file_id.trim().is_empty() {
        return Err(CommandError::DriveApi("file ID must not be empty".into()));
    }
    let mut url = url::Url::parse("https://drive.google.com/open")
        .map_err(|_| CommandError::Internal("invalid Google Drive URL".into()))?;
    url.query_pairs_mut().append_pair("id", file_id);
    Ok(url)
}

#[cfg(test)]
mod browser_url_tests {
    use super::*;

    #[test]
    fn drive_item_url_requires_id_and_keeps_untrusted_input_in_query() {
        assert!(drive_item_url("").is_err());
        assert!(drive_item_url("  ").is_err());
        let file_id = "file&next=https://example.invalid/#part";
        let url = drive_item_url(file_id).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("drive.google.com"));
        assert_eq!(url.path(), "/open");
        assert_eq!(url.fragment(), None);
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![("id".into(), file_id.into())]
        );
    }
}
