use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

use super::JobError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Draft,
    Scanning,
    ReadyForReview,
    RunningCanary,
    CanaryReview,
    Queued,
    Running,
    Pausing,
    Paused,
    Cancelling,
    Cancelled,
    Completed,
    CompletedWithErrors,
    Failed,
    AuthRequired,
    SourceRateLimited,
    WaitingForQuota,
}

impl JobStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Scanning => "SCANNING",
            Self::ReadyForReview => "READY_FOR_REVIEW",
            Self::RunningCanary => "RUNNING_CANARY",
            Self::CanaryReview => "CANARY_REVIEW",
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Pausing => "PAUSING",
            Self::Paused => "PAUSED",
            Self::Cancelling => "CANCELLING",
            Self::Cancelled => "CANCELLED",
            Self::Completed => "COMPLETED",
            Self::CompletedWithErrors => "COMPLETED_WITH_ERRORS",
            Self::Failed => "FAILED",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::SourceRateLimited => "SOURCE_RATE_LIMITED",
            Self::WaitingForQuota => "WAITING_FOR_QUOTA",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::CompletedWithErrors | Self::Cancelled | Self::Failed
        )
    }

    pub const fn is_unfinished_mutation(self) -> bool {
        matches!(
            self,
            Self::RunningCanary | Self::Running | Self::Pausing | Self::Cancelling
        )
    }

    pub const fn is_transfer_resumable(self) -> bool {
        matches!(
            self,
            Self::Paused
                | Self::Queued
                | Self::SourceRateLimited
                | Self::WaitingForQuota
                | Self::AuthRequired
        )
    }

    pub(super) const fn require_draft_pair(self) -> Result<(), JobError> {
        match self {
            Self::Draft => Ok(()),
            Self::Scanning
            | Self::ReadyForReview
            | Self::RunningCanary
            | Self::CanaryReview
            | Self::Queued
            | Self::Running
            | Self::Pausing
            | Self::Paused
            | Self::Cancelling
            | Self::Cancelled
            | Self::Completed
            | Self::CompletedWithErrors
            | Self::Failed
            | Self::AuthRequired
            | Self::SourceRateLimited
            | Self::WaitingForQuota => Err(JobError::AccountPairLocked),
        }
    }

    pub(super) const fn require_draft_roots(self) -> Result<(), JobError> {
        match self {
            Self::Draft => Ok(()),
            Self::Scanning
            | Self::ReadyForReview
            | Self::RunningCanary
            | Self::CanaryReview
            | Self::Queued
            | Self::Running
            | Self::Pausing
            | Self::Paused
            | Self::Cancelling
            | Self::Cancelled
            | Self::Completed
            | Self::CompletedWithErrors
            | Self::Failed
            | Self::AuthRequired
            | Self::SourceRateLimited
            | Self::WaitingForQuota => Err(JobError::RootsLocked),
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for JobStatus {
    type Err = JobError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DRAFT" => Ok(Self::Draft),
            "SCANNING" => Ok(Self::Scanning),
            "READY_FOR_REVIEW" => Ok(Self::ReadyForReview),
            "RUNNING_CANARY" => Ok(Self::RunningCanary),
            "CANARY_REVIEW" => Ok(Self::CanaryReview),
            "QUEUED" => Ok(Self::Queued),
            "RUNNING" => Ok(Self::Running),
            "PAUSING" => Ok(Self::Pausing),
            "PAUSED" => Ok(Self::Paused),
            "CANCELLING" => Ok(Self::Cancelling),
            "CANCELLED" => Ok(Self::Cancelled),
            "COMPLETED" => Ok(Self::Completed),
            "COMPLETED_WITH_ERRORS" => Ok(Self::CompletedWithErrors),
            "FAILED" => Ok(Self::Failed),
            "AUTH_REQUIRED" => Ok(Self::AuthRequired),
            "SOURCE_RATE_LIMITED" => Ok(Self::SourceRateLimited),
            "WAITING_FOR_QUOTA" => Ok(Self::WaitingForQuota),
            _ => Err(JobError::InvalidJobStatus),
        }
    }
}
