use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::GooglePermissionId;
use super::job::JobId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ItemId(pub u128);

impl ItemId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u128 {
        self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ItemId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemState {
    Discovered,
    Eligible,
    PendingOwnerRequired,
    PendingOwnerCreated,
    AcceptRequired,
    Accepting,
    Transferred,
    Verifying,
    Verified,
    SkippedAlreadyOwnedByTarget,
    SkippedNotOwnedBySource,
    SkippedSharedDrive,
    SkippedShortcutTarget,
    SkippedTrashed,
    SkippedIneligible,
    RetryableFailed,
    PermanentFailed,
    Cancelled,
}

impl ItemState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Discovered => "DISCOVERED",
            Self::Eligible => "ELIGIBLE",
            Self::PendingOwnerRequired => "PENDING_OWNER_REQUIRED",
            Self::PendingOwnerCreated => "PENDING_OWNER_CREATED",
            Self::AcceptRequired => "ACCEPT_REQUIRED",
            Self::Accepting => "ACCEPTING",
            Self::Transferred => "TRANSFERRED",
            Self::Verifying => "VERIFYING",
            Self::Verified => "VERIFIED",
            Self::SkippedAlreadyOwnedByTarget => "SKIPPED_ALREADY_OWNED_BY_TARGET",
            Self::SkippedNotOwnedBySource => "SKIPPED_NOT_OWNED_BY_SOURCE",
            Self::SkippedSharedDrive => "SKIPPED_SHARED_DRIVE",
            Self::SkippedShortcutTarget => "SKIPPED_SHORTCUT_TARGET",
            Self::SkippedTrashed => "SKIPPED_TRASHED",
            Self::SkippedIneligible => "SKIPPED_INELIGIBLE",
            Self::RetryableFailed => "RETRYABLE_FAILED",
            Self::PermanentFailed => "PERMANENT_FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub const fn is_skipped(self) -> bool {
        matches!(
            self,
            Self::SkippedAlreadyOwnedByTarget
                | Self::SkippedNotOwnedBySource
                | Self::SkippedSharedDrive
                | Self::SkippedShortcutTarget
                | Self::SkippedTrashed
                | Self::SkippedIneligible
        )
    }

    pub const fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

impl fmt::Display for ItemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ItemState {
    type Err = ItemError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DISCOVERED" => Ok(Self::Discovered),
            "ELIGIBLE" => Ok(Self::Eligible),
            "PENDING_OWNER_REQUIRED" => Ok(Self::PendingOwnerRequired),
            "PENDING_OWNER_CREATED" => Ok(Self::PendingOwnerCreated),
            "ACCEPT_REQUIRED" => Ok(Self::AcceptRequired),
            "ACCEPTING" => Ok(Self::Accepting),
            "TRANSFERRED" => Ok(Self::Transferred),
            "VERIFYING" => Ok(Self::Verifying),
            "VERIFIED" => Ok(Self::Verified),
            "SKIPPED_ALREADY_OWNED_BY_TARGET" => Ok(Self::SkippedAlreadyOwnedByTarget),
            "SKIPPED_NOT_OWNED_BY_SOURCE" => Ok(Self::SkippedNotOwnedBySource),
            "SKIPPED_SHARED_DRIVE" => Ok(Self::SkippedSharedDrive),
            "SKIPPED_SHORTCUT_TARGET" => Ok(Self::SkippedShortcutTarget),
            "SKIPPED_TRASHED" => Ok(Self::SkippedTrashed),
            "SKIPPED_INELIGIBLE" => Ok(Self::SkippedIneligible),
            "RETRYABLE_FAILED" => Ok(Self::RetryableFailed),
            "PERMANENT_FAILED" => Ok(Self::PermanentFailed),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(ItemError::InvalidItemState),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemError {
    InvalidItemState,
}

impl fmt::Display for ItemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidItemState => write!(f, "Invalid item state string"),
        }
    }
}

impl Error for ItemError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationItem {
    pub id: ItemId,
    pub job_id: JobId,
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub depth: i64,
    pub original_parent_ids: Vec<String>,
    pub original_owner_permission_id: Option<GooglePermissionId>,
    pub quota_bytes_used: Option<i64>,
    pub target_permission_id: Option<GooglePermissionId>,
    pub state: ItemState,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanCheckpoint {
    pub job_id: JobId,
    pub folder_id: String,
    pub page_token: Option<String>,
    pub depth: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_state_roundtrips_known_values() {
        for state in [
            ItemState::Eligible,
            ItemState::SkippedAlreadyOwnedByTarget,
            ItemState::SkippedNotOwnedBySource,
            ItemState::SkippedSharedDrive,
            ItemState::SkippedIneligible,
            ItemState::SkippedTrashed,
        ] {
            assert_eq!(ItemState::from_str(state.as_str()), Ok(state));
        }
    }

    #[test]
    fn item_state_rejects_unknown() {
        assert_eq!(
            ItemState::from_str("NOT_A_STATE"),
            Err(ItemError::InvalidItemState)
        );
    }
}
