use super::{JobError, JobStatus, MigrationJob};

impl MigrationJob {
    pub fn start_canary(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::ReadyForReview | JobStatus::Queued => {
                self.status = JobStatus::RunningCanary;
                self.queue_position = None;
                Ok(())
            }
            JobStatus::RunningCanary => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::CanaryReview
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

    pub fn complete_canary(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::RunningCanary => {
                self.status = JobStatus::CanaryReview;
                Ok(())
            }
            JobStatus::CanaryReview => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
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

    pub fn start_bulk(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::CanaryReview => {
                self.status = JobStatus::Running;
                self.queue_position = None;
                Ok(())
            }
            JobStatus::Running => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::Queued
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

    pub fn complete_transfer(
        &mut self,
        completed_at: String,
        had_errors: bool,
    ) -> Result<(), JobError> {
        match self.status {
            JobStatus::Running => {
                self.status = if had_errors {
                    JobStatus::CompletedWithErrors
                } else {
                    JobStatus::Completed
                };
                self.completed_at = Some(completed_at);
                Ok(())
            }
            JobStatus::Completed | JobStatus::CompletedWithErrors => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn pause_sharing_rate_limit(&mut self, error: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::RunningCanary | JobStatus::Running => {
                self.status = JobStatus::SourceRateLimited;
                self.last_error = Some(error);
                Ok(())
            }
            JobStatus::SourceRateLimited => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn wait_for_quota(&mut self, error: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::RunningCanary | JobStatus::Running => {
                self.status = JobStatus::WaitingForQuota;
                self.last_error = Some(error);
                Ok(())
            }
            JobStatus::WaitingForQuota => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited => Err(JobError::IllegalTransition),
        }
    }

    pub fn require_auth(&mut self, error: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::RunningCanary
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Paused
            | JobStatus::Queued
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => {
                self.status = JobStatus::AuthRequired;
                self.queue_position = None;
                self.last_error = Some(error);
                Ok(())
            }
            JobStatus::AuthRequired => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::CanaryReview
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed => Err(JobError::IllegalTransition),
        }
    }

    pub fn pause_transfer(&mut self) -> Result<(), JobError> {
        match self.status {
            JobStatus::RunningCanary
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling => {
                self.status = JobStatus::Paused;
                self.queue_position = None;
                Ok(())
            }
            JobStatus::Paused => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub fn resume_transfer(&mut self, as_canary: bool) -> Result<(), JobError> {
        match self.status {
            JobStatus::Paused
            | JobStatus::Queued
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota
            | JobStatus::AuthRequired => {
                self.status = if as_canary {
                    JobStatus::RunningCanary
                } else {
                    JobStatus::Running
                };
                self.queue_position = None;
                self.last_error = None;
                Ok(())
            }
            JobStatus::RunningCanary if as_canary => Ok(()),
            JobStatus::Running if !as_canary => Ok(()),
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed => Err(JobError::IllegalTransition),
        }
    }

    pub fn enqueue(&mut self, position: i64) -> Result<(), JobError> {
        match self.status {
            JobStatus::ReadyForReview
            | JobStatus::CanaryReview
            | JobStatus::Paused
            | JobStatus::Queued
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota
            | JobStatus::AuthRequired => {
                self.status = JobStatus::Queued;
                self.queue_position = Some(position);
                Ok(())
            }
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::RunningCanary
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed => Err(JobError::IllegalTransition),
        }
    }

    pub fn remove_from_queue(&mut self, restored_status: JobStatus) -> Result<(), JobError> {
        if self.status != JobStatus::Queued
            || !matches!(
                restored_status,
                JobStatus::ReadyForReview | JobStatus::CanaryReview | JobStatus::Paused
            )
        {
            return Err(JobError::IllegalTransition);
        }
        self.status = restored_status;
        self.queue_position = None;
        Ok(())
    }

    pub fn set_queue_position(&mut self, position: Option<i64>) {
        self.queue_position = position;
    }

    pub fn cancel_job(&mut self, completed_at: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::Draft
            | JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => {
                self.status = JobStatus::Cancelled;
                self.completed_at = Some(completed_at);
                self.queue_position = None;
                Ok(())
            }
            JobStatus::Cancelled => Ok(()),
            JobStatus::Completed | JobStatus::CompletedWithErrors | JobStatus::Failed => {
                Err(JobError::IllegalTransition)
            }
        }
    }

    pub fn set_last_error(&mut self, error: impl Into<String>) {
        self.last_error = Some(error.into());
    }
}
