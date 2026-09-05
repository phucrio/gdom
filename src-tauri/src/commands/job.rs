use tauri::State;

use crate::application::job_service::JobServiceError;
use crate::commands::dto::{
    CreateJobInput, JobDto, JobIdInput, ListJobsFilter, RemoveRootInput, RootFolderInput,
    UpdateDraftJobAccountsInput, ValidateRootResultDto,
};
use crate::commands::error::CommandError;
use crate::domain::AccountId;
use crate::domain::job::{JobId, RootId};
use crate::state::AppState;

fn parse_account_id(raw: &str) -> Result<AccountId, CommandError> {
    raw.trim()
        .parse::<u128>()
        .map(AccountId::new)
        .map_err(|_| CommandError::UnsupportedAccount("Invalid account ID format".to_owned()))
}

fn parse_job_id(raw: &str) -> Result<JobId, CommandError> {
    raw.trim()
        .parse::<u128>()
        .map(JobId::new)
        .map_err(|_| CommandError::JobNotFound("Invalid job ID format".to_owned()))
}

fn parse_root_id(raw: &str) -> Result<RootId, CommandError> {
    raw.trim()
        .parse::<u128>()
        .map(RootId::new)
        .map_err(|_| CommandError::InvalidFolder("Invalid root ID format".to_owned()))
}

fn map_job_service_error(err: JobServiceError) -> CommandError {
    match err {
        JobServiceError::SameSourceAndTarget => CommandError::SameSourceAndTarget(
            "Source and target accounts must be different".to_owned(),
        ),
        JobServiceError::SourceAccountNotFound(id) => {
            CommandError::AccountNotFound(format!("Source account {} not found", id.value()))
        }
        JobServiceError::TargetAccountNotFound(id) => {
            CommandError::AccountNotFound(format!("Target account {} not found", id.value()))
        }
        JobServiceError::AccountNotActive(id) => CommandError::UnsupportedAccount(format!(
            "Account {} is not active or connected",
            id.value()
        )),
        JobServiceError::JobNotFound(id) => {
            CommandError::JobNotFound(format!("Migration job {id} not found"))
        }
        JobServiceError::RootNotFound(id) => {
            CommandError::InvalidFolder(format!("Migration root {id} not found"))
        }
        JobServiceError::DuplicateRoot(file_id) => {
            CommandError::DuplicateRoot(format!("Root folder {file_id} already added to this job"))
        }
        JobServiceError::AccountPairLocked => CommandError::AccountPairLocked(
            "Account pair cannot be changed after scan starts".to_owned(),
        ),
        JobServiceError::RootsLocked => CommandError::RootsLocked(
            "Root folders cannot be modified after scan starts".to_owned(),
        ),
        JobServiceError::ParseError(e) => CommandError::InvalidFolder(e.to_string()),
        JobServiceError::TokenError(e) => CommandError::OAuth(e),
        JobServiceError::DriveError(e) => CommandError::Internal(e),
        JobServiceError::FolderNotFound => {
            CommandError::InvalidFolder("Folder not found in Google Drive".to_owned())
        }
        JobServiceError::NotAFolder => {
            CommandError::InvalidFolder("Selected item is not a Google Drive folder".to_owned())
        }
        JobServiceError::FolderTrashed => {
            CommandError::InvalidFolder("Folder is trashed in Google Drive".to_owned())
        }
        JobServiceError::SharedDriveNotSupported => CommandError::SharedDriveNotSupported(
            "Shared Drives cannot be selected as personal migration roots".to_owned(),
        ),
        JobServiceError::NotOwnedBySourceAccount => CommandError::NotOwnedBySource(
            "Folder is not owned by the selected source account".to_owned(),
        ),
        JobServiceError::StoreError(e) => CommandError::Database(e),
    }
}

pub(crate) async fn create_job_inner(
    state: &AppState,
    input: CreateJobInput,
) -> Result<JobDto, CommandError> {
    let source_id = parse_account_id(&input.source_account_id)?;
    let target_id = parse_account_id(&input.target_account_id)?;

    let job = state
        .job_service
        .create_job(source_id, target_id)
        .await
        .map_err(map_job_service_error)?;

    Ok(JobDto::from(&job))
}

pub(crate) async fn update_draft_job_accounts_inner(
    state: &AppState,
    input: UpdateDraftJobAccountsInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;
    let source_id = parse_account_id(&input.source_account_id)?;
    let target_id = parse_account_id(&input.target_account_id)?;

    let job = state
        .job_service
        .update_draft_job_accounts(job_id, source_id, target_id)
        .await
        .map_err(map_job_service_error)?;

    Ok(JobDto::from(&job))
}

pub(crate) async fn get_job_inner(
    state: &AppState,
    input: JobIdInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;

    let job = state
        .job_service
        .get_job(job_id)
        .await
        .map_err(map_job_service_error)?;

    Ok(JobDto::from(&job))
}

pub(crate) async fn list_jobs_inner(
    state: &AppState,
    filter: Option<ListJobsFilter>,
) -> Result<Vec<JobDto>, CommandError> {
    let jobs = state
        .job_service
        .list_jobs()
        .await
        .map_err(map_job_service_error)?;

    let dtos: Vec<JobDto> = jobs
        .into_iter()
        .filter(|j| {
            if let Some(st) = filter.as_ref().and_then(|f| f.status.as_ref()) {
                j.status().as_str().eq_ignore_ascii_case(st)
            } else {
                true
            }
        })
        .map(|j| JobDto::from(&j))
        .collect();

    Ok(dtos)
}

pub(crate) async fn validate_root_inner(
    state: &AppState,
    input: RootFolderInput,
) -> Result<ValidateRootResultDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;

    let metadata = state
        .job_service
        .validate_root(job_id, &input.input)
        .await
        .map_err(map_job_service_error)?;

    Ok(ValidateRootResultDto {
        folder_id: metadata.id,
        name: metadata.name,
    })
}

pub(crate) async fn add_root_inner(
    state: &AppState,
    input: RootFolderInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;

    let job = state
        .job_service
        .add_root(job_id, &input.input)
        .await
        .map_err(map_job_service_error)?;

    Ok(JobDto::from(&job))
}

pub(crate) async fn remove_root_inner(
    state: &AppState,
    input: RemoveRootInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;
    let root_id = parse_root_id(&input.root_id)?;

    let job = state
        .job_service
        .remove_root(job_id, root_id)
        .await
        .map_err(map_job_service_error)?;

    Ok(JobDto::from(&job))
}

#[tauri::command]
pub async fn create_job(
    state: State<'_, AppState>,
    input: CreateJobInput,
) -> Result<JobDto, CommandError> {
    create_job_inner(&state, input).await
}

#[tauri::command]
pub async fn update_draft_job_accounts(
    state: State<'_, AppState>,
    input: UpdateDraftJobAccountsInput,
) -> Result<JobDto, CommandError> {
    update_draft_job_accounts_inner(&state, input).await
}

#[tauri::command]
pub async fn get_job(
    state: State<'_, AppState>,
    input: JobIdInput,
) -> Result<JobDto, CommandError> {
    get_job_inner(&state, input).await
}

#[tauri::command]
pub async fn list_jobs(
    state: State<'_, AppState>,
    filter: Option<ListJobsFilter>,
) -> Result<Vec<JobDto>, CommandError> {
    list_jobs_inner(&state, filter).await
}

#[tauri::command]
pub async fn validate_root(
    state: State<'_, AppState>,
    input: RootFolderInput,
) -> Result<ValidateRootResultDto, CommandError> {
    validate_root_inner(&state, input).await
}

#[tauri::command]
pub async fn add_root(
    state: State<'_, AppState>,
    input: RootFolderInput,
) -> Result<JobDto, CommandError> {
    add_root_inner(&state, input).await
}

#[tauri::command]
pub async fn remove_root(
    state: State<'_, AppState>,
    input: RemoveRootInput,
) -> Result<JobDto, CommandError> {
    remove_root_inner(&state, input).await
}
