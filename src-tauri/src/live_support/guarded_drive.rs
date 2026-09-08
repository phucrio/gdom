use crate::{
    application::{AccessToken, drive_folder::*, drive_transfer::*, drive_tree::*},
    infrastructure::google_drive::GoogleDriveClient,
};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct GuardedDrive {
    pub client: GoogleDriveClient,
    pub source_token: AccessToken,
    pub target_token: AccessToken,
    pub target_email: String,
    pub allowed_ids: HashSet<String>,
    pub mutations_enabled: AtomicBool,
    pub pending_seen: AtomicBool,
    pub accept_seen: AtomicBool,
    pub target_read_seen: AtomicBool,
}
impl GuardedDrive {
    fn source(&self, token: &AccessToken) -> bool {
        token.expose_secret() == self.source_token.expose_secret()
    }
    fn target(&self, token: &AccessToken) -> bool {
        token.expose_secret() == self.target_token.expose_secret()
    }
    fn mutation_allowed(&self, file_id: &str) -> bool {
        self.mutations_enabled.load(Ordering::SeqCst) && self.allowed_ids.contains(file_id)
    }
}
impl DriveFolderLookupPort for GuardedDrive {
    fn get_folder_metadata<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
    ) -> DriveFolderLookupFuture<'a> {
        Box::pin(async move {
            if !self.source(token) || !self.allowed_ids.contains(folder_id) {
                return Err(DriveFolderLookupError::Forbidden);
            }
            DriveFolderLookupPort::get_folder_metadata(&self.client, token, folder_id).await
        })
    }
}
impl DriveTreePort for GuardedDrive {
    fn list_children<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
        page_token: Option<&'a str>,
    ) -> DriveListFuture<'a> {
        Box::pin(async move {
            if !self.source(token) || !self.allowed_ids.contains(folder_id) {
                return Err(DriveTreeError::Forbidden);
            }
            let page =
                DriveTreePort::list_children(&self.client, token, folder_id, page_token).await?;
            if page
                .files
                .iter()
                .any(|file| !self.allowed_ids.contains(&file.id))
            {
                return Err(DriveTreeError::Forbidden);
            }
            Ok(page)
        })
    }
}
impl DriveQuotaPort for GuardedDrive {
    fn get_storage_quota<'a>(&'a self, token: &'a AccessToken) -> DriveQuotaFuture<'a> {
        Box::pin(async move {
            if !self.target(token) {
                return Err(DriveTreeError::Forbidden);
            }
            DriveQuotaPort::get_storage_quota(&self.client, token).await
        })
    }
}
impl DriveTransferPort for GuardedDrive {
    fn get_file<'a>(&'a self, token: &'a AccessToken, file_id: &'a str) -> DriveFileFuture<'a> {
        Box::pin(async move {
            if !self.allowed_ids.contains(file_id) || !(self.source(token) || self.target(token)) {
                return Err(DriveTransferError::Forbidden);
            }
            if self.target(token) {
                self.target_read_seen.store(true, Ordering::SeqCst);
            }
            DriveTransferPort::get_file(&self.client, token, file_id).await
        })
    }
    fn create_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        email: &'a str,
    ) -> DrivePermissionFuture<'a> {
        Box::pin(async move {
            if !self.mutation_allowed(file_id) || !self.source(token) || email != self.target_email
            {
                return Err(DriveTransferError::Forbidden);
            }
            self.pending_seen.store(true, Ordering::SeqCst);
            DriveTransferPort::create_pending_owner(&self.client, token, file_id, email).await
        })
    }
    fn update_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        permission_id: &'a str,
    ) -> DrivePermissionFuture<'a> {
        Box::pin(async move {
            if !self.mutation_allowed(file_id) || !self.source(token) {
                return Err(DriveTransferError::Forbidden);
            }
            self.pending_seen.store(true, Ordering::SeqCst);
            DriveTransferPort::update_pending_owner(&self.client, token, file_id, permission_id)
                .await
        })
    }
    fn accept_ownership<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        permission_id: &'a str,
    ) -> DrivePermissionFuture<'a> {
        Box::pin(async move {
            if !self.mutation_allowed(file_id) || !self.target(token) {
                return Err(DriveTransferError::Forbidden);
            }
            self.accept_seen.store(true, Ordering::SeqCst);
            DriveTransferPort::accept_ownership(&self.client, token, file_id, permission_id).await
        })
    }
}

#[tokio::test]
async fn wrong_token_or_unapproved_item_never_reaches_network() {
    let guard = GuardedDrive {
        client: GoogleDriveClient::for_test("http://127.0.0.1:1".into()).unwrap(),
        source_token: AccessToken::new("source".into()),
        target_token: AccessToken::new("target".into()),
        target_email: "b@gmail.com".into(),
        allowed_ids: HashSet::from(["fixture".into()]),
        mutations_enabled: AtomicBool::new(false),
        pending_seen: AtomicBool::new(false),
        accept_seen: AtomicBool::new(false),
        target_read_seen: AtomicBool::new(false),
    };
    assert_eq!(
        guard
            .create_pending_owner(&guard.source_token, "fixture", "b@gmail.com")
            .await,
        Err(DriveTransferError::Forbidden)
    );
    guard.mutations_enabled.store(true, Ordering::SeqCst);
    assert_eq!(
        guard
            .create_pending_owner(&guard.target_token, "fixture", "b@gmail.com")
            .await,
        Err(DriveTransferError::Forbidden)
    );
    assert_eq!(
        guard
            .accept_ownership(&guard.source_token, "fixture", "permission")
            .await,
        Err(DriveTransferError::Forbidden)
    );
    assert_eq!(
        guard
            .accept_ownership(&guard.target_token, "outside", "permission")
            .await,
        Err(DriveTransferError::Forbidden)
    );
    assert!(!guard.pending_seen.load(Ordering::SeqCst));
    assert!(!guard.accept_seen.load(Ordering::SeqCst));
}
