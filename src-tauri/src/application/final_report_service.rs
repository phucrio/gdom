use super::*;
use crate::application::final_report::FinalReportExport;

impl<A, J> JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    pub async fn export_final_report(
        &self,
        job_id: JobId,
        destination: &str,
    ) -> Result<FinalReportExport, JobServiceError> {
        let path = Path::new(destination);
        if destination.trim().is_empty()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("csv") || extension.eq_ignore_ascii_case("txt")
                })
        {
            return Err(JobServiceError::ExportFailed(
                "choose a .txt or .csv destination".into(),
            ));
        }
        if self.scan_is_in_flight(job_id) || self.transfer_is_in_flight(job_id) {
            return Err(JobServiceError::TransferInProgress);
        }
        let job = self.get_job(job_id).await?;
        if !matches!(
            job.status(),
            JobStatus::Completed
                | JobStatus::CompletedWithErrors
                | JobStatus::Cancelled
                | JobStatus::Failed
        ) {
            return Err(JobServiceError::IllegalTransition);
        }
        let snapshot = self.job_store.final_report_snapshot(job_id).await?;
        let result = FinalReportExport {
            path: destination.to_owned(),
            status: snapshot.status,
            counts: snapshot.counts(),
        };
        let report = snapshot.render(destination_is_csv(destination));
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| JobServiceError::ExportFailed(error.to_string()))?;
        }
        std::fs::write(path, report)
            .map_err(|error| JobServiceError::ExportFailed(error.to_string()))?;
        Ok(result)
    }
}
