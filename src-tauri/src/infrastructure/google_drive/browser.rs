use super::GoogleDriveClient;
use crate::application::{
    AccessToken, DriveChildPage,
    drive_browser::{BrowseFolderRequest, BrowserFuture, DriveBrowserPort, RenameFileRequest},
};
impl DriveBrowserPort for GoogleDriveClient {
    fn list_files<'a>(
        &'a self,
        token: &'a AccessToken,
        request: &'a BrowseFolderRequest,
    ) -> BrowserFuture<'a, DriveChildPage> {
        Box::pin(async move {
            Ok(self
                .list_browse_children(
                    token,
                    request.folder_id.as_deref(),
                    request.page_token.as_deref(),
                    request.page_size,
                    request.order_by.as_deref(),
                )
                .await?)
        })
    }
    fn rename_file<'a>(
        &'a self,
        token: &'a AccessToken,
        request: &'a RenameFileRequest,
    ) -> BrowserFuture<'a, ()> {
        Box::pin(async move {
            Ok(self
                .rename_file(token, &request.file_id, &request.new_name)
                .await?)
        })
    }
    fn trash_file<'a>(&'a self, token: &'a AccessToken, file_id: &'a str) -> BrowserFuture<'a, ()> {
        Box::pin(async move { Ok(self.trash_file(token, file_id).await?) })
    }
}
