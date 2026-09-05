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

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Verified
                | Self::SkippedAlreadyOwnedByTarget
                | Self::SkippedNotOwnedBySource
                | Self::SkippedSharedDrive
                | Self::SkippedShortcutTarget
                | Self::SkippedTrashed
                | Self::SkippedIneligible
                | Self::PermanentFailed
                | Self::Cancelled
        )
    }

    pub const fn is_transfer_active(self) -> bool {
        matches!(
            self,
            Self::Eligible
                | Self::PendingOwnerRequired
                | Self::PendingOwnerCreated
                | Self::AcceptRequired
                | Self::Accepting
                | Self::Transferred
                | Self::Verifying
                | Self::RetryableFailed
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (Self::Discovered, Self::Eligible) => true,
            (Self::Eligible, Self::PendingOwnerRequired)
            | (Self::Eligible, Self::Verifying)
            | (Self::Eligible, Self::SkippedAlreadyOwnedByTarget)
            | (Self::Eligible, Self::SkippedNotOwnedBySource)
            | (Self::Eligible, Self::SkippedSharedDrive)
            | (Self::Eligible, Self::SkippedTrashed)
            | (Self::Eligible, Self::SkippedIneligible)
            | (Self::Eligible, Self::PermanentFailed)
            | (Self::Eligible, Self::RetryableFailed)
            | (Self::Eligible, Self::Cancelled) => true,
            (Self::PendingOwnerRequired, Self::PendingOwnerCreated)
            | (Self::PendingOwnerRequired, Self::Verifying)
            | (Self::PendingOwnerRequired, Self::RetryableFailed)
            | (Self::PendingOwnerRequired, Self::PermanentFailed)
            | (Self::PendingOwnerRequired, Self::Cancelled)
            | (Self::PendingOwnerRequired, Self::SkippedTrashed)
            | (Self::PendingOwnerRequired, Self::SkippedSharedDrive)
            | (Self::PendingOwnerRequired, Self::SkippedNotOwnedBySource) => true,
            (Self::PendingOwnerCreated, Self::AcceptRequired)
            | (Self::PendingOwnerCreated, Self::Verifying)
            | (Self::PendingOwnerCreated, Self::RetryableFailed)
            | (Self::PendingOwnerCreated, Self::PermanentFailed)
            | (Self::PendingOwnerCreated, Self::Cancelled)
            | (Self::PendingOwnerCreated, Self::SkippedTrashed)
            | (Self::PendingOwnerCreated, Self::SkippedSharedDrive) => true,
            (Self::AcceptRequired, Self::Accepting)
            | (Self::AcceptRequired, Self::Verifying)
            | (Self::AcceptRequired, Self::RetryableFailed)
            | (Self::AcceptRequired, Self::PermanentFailed)
            | (Self::AcceptRequired, Self::Cancelled)
            | (Self::AcceptRequired, Self::SkippedTrashed) => true,
            (Self::Accepting, Self::Transferred)
            | (Self::Accepting, Self::Verifying)
            | (Self::Accepting, Self::RetryableFailed)
            | (Self::Accepting, Self::PermanentFailed)
            | (Self::Accepting, Self::Cancelled)
            | (Self::Accepting, Self::SkippedTrashed) => true,
            (Self::Transferred, Self::Verifying)
            | (Self::Transferred, Self::RetryableFailed)
            | (Self::Transferred, Self::PermanentFailed)
            | (Self::Transferred, Self::Cancelled)
            | (Self::Transferred, Self::SkippedTrashed) => true,
            (Self::Verifying, Self::Verified)
            | (Self::Verifying, Self::RetryableFailed)
            | (Self::Verifying, Self::PermanentFailed)
            | (Self::Verifying, Self::Cancelled)
            | (Self::Verifying, Self::SkippedTrashed) => true,
            (Self::RetryableFailed, Self::PendingOwnerRequired)
            | (Self::RetryableFailed, Self::Verifying)
            | (Self::RetryableFailed, Self::PermanentFailed)
            | (Self::RetryableFailed, Self::Cancelled) => true,
            (from, to) if from == to => true,
            _ => false,
        }
    }

    pub fn transition_to(self, next: Self) -> Result<Self, ItemError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(ItemError::IllegalTransition)
        }
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
    IllegalTransition,
}

impl fmt::Display for ItemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidItemState => write!(f, "Invalid item state string"),
            Self::IllegalTransition => write!(f, "Illegal item state transition"),
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
            ItemState::Discovered,
            ItemState::Eligible,
            ItemState::PendingOwnerRequired,
            ItemState::PendingOwnerCreated,
            ItemState::AcceptRequired,
            ItemState::Accepting,
            ItemState::Transferred,
            ItemState::Verifying,
            ItemState::Verified,
            ItemState::SkippedAlreadyOwnedByTarget,
            ItemState::SkippedNotOwnedBySource,
            ItemState::SkippedSharedDrive,
            ItemState::SkippedIneligible,
            ItemState::SkippedTrashed,
            ItemState::RetryableFailed,
            ItemState::PermanentFailed,
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

    #[test]
    fn transfer_chain_allows_accept_required_and_rejects_skips() {
        let chain = [
            ItemState::Eligible,
            ItemState::PendingOwnerRequired,
            ItemState::PendingOwnerCreated,
            ItemState::AcceptRequired,
            ItemState::Accepting,
            ItemState::Transferred,
            ItemState::Verifying,
            ItemState::Verified,
        ];
        for window in chain.windows(2) {
            assert_eq!(window[0].transition_to(window[1]), Ok(window[1]));
        }
        assert_eq!(
            ItemState::Verified.transition_to(ItemState::Eligible),
            Err(ItemError::IllegalTransition)
        );
        assert!(ItemState::Verified.is_terminal());
        assert!(ItemState::Eligible.is_transfer_active());
    }
}
