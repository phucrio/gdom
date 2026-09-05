use crate::application::drive_tree::{DriveChild, FOLDER_MIME_TYPE, SHORTCUT_MIME_TYPE};
use crate::domain::GooglePermissionId;
use crate::domain::item::ItemState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanDisposition {
    EligibleFolder,
    EligibleFile,
    Shortcut,
    AlreadyOwnedByTarget,
    NotOwnedBySource,
    SharedDrive,
    Trashed,
}

impl ScanDisposition {
    pub const fn item_state(self) -> ItemState {
        match self {
            Self::EligibleFolder | Self::EligibleFile => ItemState::Eligible,
            Self::Shortcut => ItemState::SkippedIneligible,
            Self::AlreadyOwnedByTarget => ItemState::SkippedAlreadyOwnedByTarget,
            Self::NotOwnedBySource => ItemState::SkippedNotOwnedBySource,
            Self::SharedDrive => ItemState::SkippedSharedDrive,
            Self::Trashed => ItemState::SkippedTrashed,
        }
    }

    pub const fn should_recurse(self) -> bool {
        matches!(self, Self::EligibleFolder)
    }
}

pub fn classify_drive_child(
    child: &DriveChild,
    source_permission_id: &GooglePermissionId,
    target_permission_id: &GooglePermissionId,
) -> ScanDisposition {
    if child.trashed {
        return ScanDisposition::Trashed;
    }

    if child.drive_id.as_deref().is_some_and(|id| !id.is_empty()) {
        return ScanDisposition::SharedDrive;
    }

    let owned_by_target = child
        .owners
        .iter()
        .any(|owner| &owner.permission_id == target_permission_id);
    if owned_by_target {
        return ScanDisposition::AlreadyOwnedByTarget;
    }

    let owned_by_source = child
        .owners
        .iter()
        .any(|owner| &owner.permission_id == source_permission_id);
    if !owned_by_source {
        return ScanDisposition::NotOwnedBySource;
    }

    if child.mime_type == SHORTCUT_MIME_TYPE || child.shortcut_target_id.is_some() {
        return ScanDisposition::Shortcut;
    }

    if child.mime_type == FOLDER_MIME_TYPE {
        ScanDisposition::EligibleFolder
    } else {
        ScanDisposition::EligibleFile
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::drive_folder::DriveFolderOwner;

    fn child(mime: &str) -> DriveChild {
        DriveChild {
            id: "file-1".into(),
            name: "Item".into(),
            mime_type: mime.into(),
            parents: vec!["parent".into()],
            owners: vec![DriveFolderOwner {
                permission_id: GooglePermissionId::new("perm-source"),
                email_address: None,
            }],
            drive_id: None,
            quota_bytes_used: Some(10),
            trashed: false,
            shortcut_target_id: None,
        }
    }

    fn source() -> GooglePermissionId {
        GooglePermissionId::new("perm-source")
    }

    fn target() -> GooglePermissionId {
        GooglePermissionId::new("perm-target")
    }

    #[test]
    fn skips_trashed_before_other_rules() {
        let mut item = child(FOLDER_MIME_TYPE);
        item.trashed = true;
        item.drive_id = Some("drive".into());
        assert_eq!(
            classify_drive_child(&item, &source(), &target()),
            ScanDisposition::Trashed
        );
    }

    #[test]
    fn skips_shared_drive_items() {
        let mut item = child(FOLDER_MIME_TYPE);
        item.drive_id = Some("0AShared".into());
        assert_eq!(
            classify_drive_child(&item, &source(), &target()),
            ScanDisposition::SharedDrive
        );
    }

    #[test]
    fn skips_target_owned_items() {
        let mut item = child("text/plain");
        item.owners = vec![DriveFolderOwner {
            permission_id: GooglePermissionId::new("perm-target"),
            email_address: None,
        }];
        assert_eq!(
            classify_drive_child(&item, &source(), &target()),
            ScanDisposition::AlreadyOwnedByTarget
        );
    }

    #[test]
    fn skips_items_not_owned_by_source() {
        let mut item = child("text/plain");
        item.owners = vec![DriveFolderOwner {
            permission_id: GooglePermissionId::new("someone-else"),
            email_address: None,
        }];
        assert_eq!(
            classify_drive_child(&item, &source(), &target()),
            ScanDisposition::NotOwnedBySource
        );
    }

    #[test]
    fn records_source_owned_shortcut_without_eligible_folder() {
        let mut item = child(SHORTCUT_MIME_TYPE);
        item.shortcut_target_id = Some("target-folder".into());
        let disposition = classify_drive_child(&item, &source(), &target());
        assert_eq!(disposition, ScanDisposition::Shortcut);
        assert!(!disposition.should_recurse());
        assert_eq!(disposition.item_state(), ItemState::SkippedIneligible);
    }

    #[test]
    fn classifies_source_owned_folder_and_file() {
        assert_eq!(
            classify_drive_child(&child(FOLDER_MIME_TYPE), &source(), &target()),
            ScanDisposition::EligibleFolder
        );
        assert_eq!(
            classify_drive_child(&child("text/plain"), &source(), &target()),
            ScanDisposition::EligibleFile
        );
    }
}
