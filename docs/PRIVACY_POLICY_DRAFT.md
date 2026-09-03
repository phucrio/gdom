# Privacy Policy draft

Status: draft for legal and release review. Publish a stable URL and add it to the production OAuth consent screen before public release.

## Data GDOM accesses

GDOM accesses the Google Drive account identity, selected root and descendant metadata, permissions, and ownership state needed to scan and transfer ownership. It does not download, upload, copy, or analyze file content.

GDOM requests the full Google Drive scope so it can list and transfer arbitrary existing items selected for migration. The narrower per-file scope cannot provide the complete Drive listing required by this workflow.

## Local storage

Refresh tokens are stored only in the operating-system keychain. Account registry records, job checkpoints, and the minimum Drive metadata required to resume a job are stored in the local SQLite database on the user's device.

## No remote processing

GDOM has no backend that receives OAuth tokens or Drive metadata. It does not send Drive metadata, file content, credentials, or derived data to analytics or AI systems.

## Deletion

Users can delete local account data. This removes the selected account credential from the OS keychain and its local account/job metadata after confirmation. It does not revoke Google authorization unless the user explicitly selects a separate revoke action.

## Google API Limited Use

GDOM's use of information received from Google APIs will adhere to the Google API Services User Data Policy, including the Limited Use requirements.
