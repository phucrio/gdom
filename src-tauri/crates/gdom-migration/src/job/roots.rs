use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

use super::{JobError, JobId, MigrationJob};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RootId(pub u128);

impl RootId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u128 {
        self.0
    }
}

impl fmt::Display for RootId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RootId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RootValidationStatus {
    Validated,
    Pending,
    Failed,
}

impl RootValidationStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Validated => "VALIDATED",
            Self::Pending => "PENDING",
            Self::Failed => "FAILED",
        }
    }
}

impl fmt::Display for RootValidationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RootValidationStatus {
    type Err = JobError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "VALIDATED" => Ok(Self::Validated),
            "PENDING" => Ok(Self::Pending),
            "FAILED" => Ok(Self::Failed),
            _ => Err(JobError::InvalidRootValidationStatus),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MigrationRoot {
    pub id: RootId,
    pub job_id: JobId,
    pub root_file_id: String,
    pub root_name: String,
    pub validation_status: RootValidationStatus,
    pub created_at: String,
}

impl MigrationJob {
    pub fn add_root(&mut self, root: MigrationRoot) -> Result<(), JobError> {
        self.status.require_draft_roots()?;
        if self
            .roots
            .iter()
            .any(|r| r.root_file_id == root.root_file_id)
        {
            return Err(JobError::DuplicateRoot(root.root_file_id));
        }
        self.roots.push(root);
        Ok(())
    }

    pub fn remove_root(&mut self, root_id: RootId) -> Result<(), JobError> {
        self.status.require_draft_roots()?;
        let pos = self
            .roots
            .iter()
            .position(|r| r.id == root_id)
            .ok_or(JobError::RootNotFound(root_id))?;
        self.roots.remove(pos);
        Ok(())
    }

    pub(super) fn has_validated_root(&self) -> bool {
        self.roots
            .iter()
            .any(|root| root.validation_status == RootValidationStatus::Validated)
    }
}
