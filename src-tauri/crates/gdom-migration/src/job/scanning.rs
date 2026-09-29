use super::{JobError, JobStatus, MigrationJob};

impl MigrationJob {
    pub fn start_scanning(&mut self, started_at: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::Draft => {
                if !self.has_validated_root() {
                    return Err(JobError::NoValidatedRoots);
                }
                self.status = JobStatus::Scanning;
                if self.started_at.is_none() {
                    self.started_at = Some(started_at);
                }
                Ok(())
            }
            JobStatus::Paused => {
                self.status = JobStatus::Scanning;
                Ok(())
            }
            JobStatus::Scanning => Ok(()),
            JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn pause_scanning(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::Scanning => {
                self.status = JobStatus::Paused;
                Ok(())
            }
            JobStatus::Paused => Ok(()),
            JobStatus::Draft
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn complete_scanning(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::Scanning => {
                self.status = JobStatus::ReadyForReview;
                Ok(())
            }
            JobStatus::ReadyForReview => Ok(()),
            JobStatus::Draft
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn fail_scanning(&mut self, error: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::Scanning | JobStatus::Paused => {
                self.status = JobStatus::Failed;
                self.last_error = Some(error);
                Ok(())
            }
            JobStatus::Draft
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }
}
