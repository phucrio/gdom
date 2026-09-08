use tauri::State;

use crate::application::final_report::FinalReportExport;
use crate::commands::dto::{ExportDryRunInput, JobDto, JobIdInput};
use crate::commands::error::CommandError;
use crate::commands::job::{job_dto_with_scan, map_job_service_error, parse_job_id};
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderQueuedJobInput {
    pub job_id: String,
    pub position: i64,
}

pub(crate) async fn reorder_queued_job_inner(
    state: &AppState,
    input: ReorderQueuedJobInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;
    let job = state
        .job_service
        .reorder_queued_job(job_id, input.position)
        .await
        .map_err(map_job_service_error)?;
    job_dto_with_scan(state, job).await
}

pub(crate) async fn remove_queued_job_inner(
    state: &AppState,
    input: JobIdInput,
) -> Result<JobDto, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;
    let job = state
        .job_service
        .remove_queued_job(job_id)
        .await
        .map_err(map_job_service_error)?;
    job_dto_with_scan(state, job).await
}

#[tauri::command]
pub async fn reorder_queued_job(
    state: State<'_, AppState>,
    input: ReorderQueuedJobInput,
) -> Result<JobDto, CommandError> {
    reorder_queued_job_inner(&state, input).await
}

#[tauri::command]
pub async fn remove_queued_job(
    state: State<'_, AppState>,
    input: JobIdInput,
) -> Result<JobDto, CommandError> {
    remove_queued_job_inner(&state, input).await
}

pub(crate) async fn export_final_report_inner(
    state: &AppState,
    input: ExportDryRunInput,
) -> Result<FinalReportExport, CommandError> {
    let job_id = parse_job_id(&input.job_id)?;
    state
        .job_service
        .export_final_report(job_id, &input.destination)
        .await
        .map_err(map_job_service_error)
}

#[tauri::command]
pub async fn export_final_report(
    state: State<'_, AppState>,
    input: ExportDryRunInput,
) -> Result<FinalReportExport, CommandError> {
    export_final_report_inner(&state, input).await
}

#[cfg(test)]
mod tests {
    use super::ReorderQueuedJobInput;

    #[test]
    fn reorder_rejects_missing_fractional_or_out_of_range_integer_input() {
        for input in [
            r#"{"jobId":"1"}"#,
            r#"{"jobId":"1","position":1.5}"#,
            r#"{"jobId":"1","position":18446744073709551615}"#,
        ] {
            assert!(serde_json::from_str::<ReorderQueuedJobInput>(input).is_err());
        }
    }
}
