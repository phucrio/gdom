use serde::Serialize;

use crate::application::AccessToken;
use crate::application::drive_folder::DriveFolderOwner;
use crate::application::drive_transfer::{
    DriveFileFuture, DriveFileSnapshot, DrivePermission, DrivePermissionFuture, DriveTransferError,
    DriveTransferFailure, DriveTransferPort,
};
use crate::domain::GooglePermissionId;

use super::{
    GoogleDriveClient, GoogleDriveError, GoogleDriveResponseError, RawFileResponse, RawPermission,
    encode_path_segment,
};

const FILE_FIELDS: &str = "id,name,mimeType,parents,owners(permissionId,emailAddress),trashed,driveId,quotaBytesUsed,size,permissions(id,type,role,emailAddress,pendingOwner)";

impl GoogleDriveClient {
    pub async fn get_file(
        &self,
        token: &AccessToken,
        file_id: &str,
    ) -> Result<DriveFileSnapshot, GoogleDriveError> {
        self.get_file_response(token, file_id)
            .await
            .map_err(|failure| failure.error)
    }

    async fn get_file_for_transfer(
        &self,
        token: &AccessToken,
        file_id: &str,
    ) -> Result<DriveFileSnapshot, DriveTransferFailure> {
        self.get_file_response(token, file_id)
            .await
            .map_err(DriveTransferFailure::from)
    }

    async fn get_file_response(
        &self,
        token: &AccessToken,
        file_id: &str,
    ) -> Result<DriveFileSnapshot, GoogleDriveResponseError> {
        let query = {
            let mut encoded = url::form_urlencoded::Serializer::new(String::new());
            encoded.append_pair("supportsAllDrives", "true");
            encoded.append_pair("fields", FILE_FIELDS);
            encoded.finish()
        };
        let url = format!(
            "{}/drive/v3/files/{}?{query}",
            self.base_url,
            encode_path_segment(file_id)
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| GoogleDriveResponseError::from_error(GoogleDriveError::Transport))?;
        if !response.status().is_success() {
            return Err(Self::response_error(response).await);
        }
        let raw = response
            .json::<RawFileResponse>()
            .await
            .map_err(|_| GoogleDriveResponseError::from_error(GoogleDriveError::InvalidResponse))?;
        Ok(drive_file_snapshot_from_raw(raw))
    }

    pub async fn create_pending_owner(
        &self,
        token: &AccessToken,
        file_id: &str,
        email: &str,
    ) -> Result<DrivePermission, GoogleDriveError> {
        self.create_pending_owner_response(token, file_id, email)
            .await
            .map_err(|failure| failure.error)
    }

    async fn create_pending_owner_for_transfer(
        &self,
        token: &AccessToken,
        file_id: &str,
        email: &str,
    ) -> Result<DrivePermission, DriveTransferFailure> {
        self.create_pending_owner_response(token, file_id, email)
            .await
            .map_err(DriveTransferFailure::from)
    }

    async fn create_pending_owner_response(
        &self,
        token: &AccessToken,
        file_id: &str,
        email: &str,
    ) -> Result<DrivePermission, GoogleDriveResponseError> {
        let url = format!(
            "{}/drive/v3/files/{}/permissions?sendNotificationEmail=true&supportsAllDrives=true",
            self.base_url,
            encode_path_segment(file_id)
        );
        let body = PendingOwnerCreateBody {
            type_: "user",
            role: "writer",
            email_address: email,
            pending_owner: true,
        };
        self.send_permission_mutation(token, self.client.post(url), &body)
            .await
    }

    pub async fn update_pending_owner(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, GoogleDriveError> {
        self.update_pending_owner_response(token, file_id, permission_id)
            .await
            .map_err(|failure| failure.error)
    }

    async fn update_pending_owner_for_transfer(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, DriveTransferFailure> {
        self.update_pending_owner_response(token, file_id, permission_id)
            .await
            .map_err(DriveTransferFailure::from)
    }

    async fn update_pending_owner_response(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, GoogleDriveResponseError> {
        let url = format!(
            "{}/drive/v3/files/{}/permissions/{}?sendNotificationEmail=true&supportsAllDrives=true",
            self.base_url,
            encode_path_segment(file_id),
            encode_path_segment(permission_id)
        );
        let body = PendingOwnerUpdateBody {
            role: "writer",
            pending_owner: true,
        };
        self.send_permission_mutation(token, self.client.patch(url), &body)
            .await
    }

    pub async fn accept_ownership(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, GoogleDriveError> {
        self.accept_ownership_response(token, file_id, permission_id)
            .await
            .map_err(|failure| failure.error)
    }

    async fn accept_ownership_for_transfer(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, DriveTransferFailure> {
        self.accept_ownership_response(token, file_id, permission_id)
            .await
            .map_err(DriveTransferFailure::from)
    }

    async fn accept_ownership_response(
        &self,
        token: &AccessToken,
        file_id: &str,
        permission_id: &str,
    ) -> Result<DrivePermission, GoogleDriveResponseError> {
        let url = format!(
            "{}/drive/v3/files/{}/permissions/{}?transferOwnership=true&supportsAllDrives=true",
            self.base_url,
            encode_path_segment(file_id),
            encode_path_segment(permission_id)
        );
        let body = AcceptOwnershipBody { role: "owner" };
        self.send_permission_mutation(token, self.client.patch(url), &body)
            .await
    }

    async fn send_permission_mutation<B: Serialize>(
        &self,
        token: &AccessToken,
        builder: reqwest::RequestBuilder,
        body: &B,
    ) -> Result<DrivePermission, GoogleDriveResponseError> {
        let response = builder
            .bearer_auth(token.expose_secret())
            .json(body)
            .send()
            .await
            .map_err(|_| GoogleDriveResponseError::from_error(GoogleDriveError::Transport))?;
        if !response.status().is_success() {
            return Err(Self::response_error(response).await);
        }
        let raw = response
            .json::<RawPermission>()
            .await
            .map_err(|_| GoogleDriveResponseError::from_error(GoogleDriveError::InvalidResponse))?;
        Ok(drive_permission_from_raw(raw))
    }
}

impl DriveTransferPort for GoogleDriveClient {
    fn get_file<'a>(&'a self, token: &'a AccessToken, file_id: &'a str) -> DriveFileFuture<'a> {
        Box::pin(async move { self.get_file_for_transfer(token, file_id).await })
    }

    fn create_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        email: &'a str,
    ) -> DrivePermissionFuture<'a> {
        Box::pin(async move {
            self.create_pending_owner_for_transfer(token, file_id, email)
                .await
        })
    }

    fn update_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        permission_id: &'a str,
    ) -> DrivePermissionFuture<'a> {
        Box::pin(async move {
            self.update_pending_owner_for_transfer(token, file_id, permission_id)
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
            self.accept_ownership_for_transfer(token, file_id, permission_id)
                .await
        })
    }
}

impl From<GoogleDriveResponseError> for DriveTransferFailure {
    fn from(failure: GoogleDriveResponseError) -> Self {
        let kind = DriveTransferError::from(failure.error);
        match failure.http_status {
            Some(http_status) => Self::from_api_response(
                kind,
                http_status,
                failure.google_status,
                failure.reasons,
                failure.message,
            ),
            None => Self::from(kind),
        }
    }
}

impl From<GoogleDriveError> for DriveTransferError {
    fn from(error: GoogleDriveError) -> Self {
        match error {
            GoogleDriveError::Unauthorized => Self::Unauthorized,
            GoogleDriveError::Forbidden
            | GoogleDriveError::ApiNotEnabled
            | GoogleDriveError::InsufficientScope => Self::Forbidden,
            GoogleDriveError::NotFound => Self::NotFound,
            GoogleDriveError::RateLimited => Self::RateLimited,
            GoogleDriveError::SharingRateLimitExceeded => Self::SharingRateLimitExceeded,
            GoogleDriveError::StorageQuotaExceeded => Self::StorageQuotaExceeded,
            GoogleDriveError::ServerUnavailable => Self::ServerUnavailable,
            GoogleDriveError::Transport => Self::Transport,
            GoogleDriveError::InvalidResponse => Self::InvalidResponse,
            GoogleDriveError::UnexpectedStatus(status) => Self::UnexpectedStatus(status),
        }
    }
}

fn drive_permission_from_raw(raw: RawPermission) -> DrivePermission {
    DrivePermission {
        id: raw.id,
        role: raw.role.unwrap_or_default(),
        type_: raw.type_.unwrap_or_default(),
        email_address: raw.email_address,
        pending_owner: raw.pending_owner.unwrap_or(false),
    }
}

fn drive_file_snapshot_from_raw(raw: RawFileResponse) -> DriveFileSnapshot {
    DriveFileSnapshot {
        id: raw.id,
        name: raw.name,
        mime_type: raw.mime_type,
        parents: raw.parents.unwrap_or_default(),
        owners: raw
            .owners
            .unwrap_or_default()
            .into_iter()
            .map(|owner| DriveFolderOwner {
                permission_id: GooglePermissionId::new(owner.permission_id),
                email_address: owner.email_address,
                avatar_url: None,
            })
            .collect(),
        trashed: raw.trashed.unwrap_or(false),
        drive_id: raw.drive_id,
        quota_bytes_used: super::parse_i64_string(raw.quota_bytes_used.or(raw.size)),
        permissions: raw
            .permissions
            .unwrap_or_default()
            .into_iter()
            .map(drive_permission_from_raw)
            .collect(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingOwnerCreateBody<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    role: &'static str,
    email_address: &'a str,
    pending_owner: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingOwnerUpdateBody {
    role: &'static str,
    pending_owner: bool,
}

#[derive(Serialize)]
struct AcceptOwnershipBody {
    role: &'static str,
}
