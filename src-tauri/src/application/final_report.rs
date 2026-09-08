use serde::Serialize;

use crate::application::preflight::csv_cell;
use crate::domain::item::ItemState;
use crate::domain::job::{JobId, JobStatus};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalReportCounts {
    pub total: u64,
    pub verified: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub skipped: u64,
    pub unfinished: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalReportExport {
    pub path: String,
    pub status: JobStatus,
    pub counts: FinalReportCounts,
}

pub struct FinalReportSnapshot {
    pub job_id: JobId,
    pub status: JobStatus,
    pub source_email: String,
    pub source_display_name: String,
    pub target_email: String,
    pub target_display_name: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_error: Option<String>,
    pub roots: Vec<(String, String)>,
    pub items: Vec<FinalReportItem>,
}

pub struct FinalReportItem {
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub depth: i64,
    pub quota_bytes_used: Option<i64>,
    pub original_parent_ids: Vec<String>,
    pub state: ItemState,
    pub error_code: Option<String>,
    pub error_reason: Option<String>,
    pub error_message: Option<String>,
    pub transferred_at: Option<String>,
    pub verified_at: Option<String>,
}

impl FinalReportSnapshot {
    pub fn counts(&self) -> FinalReportCounts {
        let mut counts = FinalReportCounts::default();
        for item in &self.items {
            counts.total += 1;
            match item.state {
                ItemState::Verified => counts.verified += 1,
                ItemState::RetryableFailed | ItemState::PermanentFailed => counts.failed += 1,
                ItemState::Cancelled => counts.cancelled += 1,
                ItemState::SkippedAlreadyOwnedByTarget
                | ItemState::SkippedNotOwnedBySource
                | ItemState::SkippedSharedDrive
                | ItemState::SkippedShortcutTarget
                | ItemState::SkippedTrashed
                | ItemState::SkippedIneligible => counts.skipped += 1,
                ItemState::Discovered
                | ItemState::Eligible
                | ItemState::PendingOwnerRequired
                | ItemState::PendingOwnerCreated
                | ItemState::AcceptRequired
                | ItemState::Accepting
                | ItemState::Transferred
                | ItemState::Verifying => counts.unfinished += 1,
            }
        }
        counts
    }

    pub fn render(&self, csv: bool) -> String {
        let counts = self.counts();
        let mut output = String::new();
        let metadata = [
            ("job_id", self.job_id.to_string()),
            ("status", self.status.as_str().to_owned()),
            ("source_email", self.source_email.clone()),
            ("source_display_name", self.source_display_name.clone()),
            ("target_email", self.target_email.clone()),
            ("target_display_name", self.target_display_name.clone()),
            ("created_at", self.created_at.clone()),
            ("started_at", self.started_at.clone().unwrap_or_default()),
            (
                "completed_at",
                self.completed_at.clone().unwrap_or_default(),
            ),
            ("last_error", self.last_error.clone().unwrap_or_default()),
            ("total", counts.total.to_string()),
            ("verified", counts.verified.to_string()),
            ("failed", counts.failed.to_string()),
            ("cancelled", counts.cancelled.to_string()),
            ("skipped", counts.skipped.to_string()),
            ("unfinished", counts.unfinished.to_string()),
        ];
        for (name, value) in &metadata {
            append_row(&mut output, &[name, value], csv);
        }
        output.push('\n');
        append_row(&mut output, &["root_file_id", "root_name"], csv);
        for (file_id, name) in &self.roots {
            append_row(&mut output, &[file_id, name], csv);
        }
        output.push('\n');
        append_row(
            &mut output,
            &[
                "file_id",
                "name",
                "mime_type",
                "depth",
                "quota_bytes_used",
                "original_parent_ids",
                "state",
                "error_code",
                "error_reason",
                "error_message",
                "transferred_at",
                "verified_at",
            ],
            csv,
        );
        for item in &self.items {
            append_row(
                &mut output,
                &[
                    &item.file_id,
                    &item.name,
                    &item.mime_type,
                    &item.depth.to_string(),
                    &item
                        .quota_bytes_used
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    &item.original_parent_ids.join(";"),
                    item.state.as_str(),
                    item.error_code.as_deref().unwrap_or_default(),
                    item.error_reason.as_deref().unwrap_or_default(),
                    item.error_message.as_deref().unwrap_or_default(),
                    item.transferred_at.as_deref().unwrap_or_default(),
                    item.verified_at.as_deref().unwrap_or_default(),
                ],
                csv,
            );
        }
        output
    }
}

fn append_row(output: &mut String, values: &[&str], csv: bool) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(if csv { "," } else { "\t" });
        }
        let sanitized = gdom_logs::redact_secrets(value);
        if csv {
            output.push_str(&csv_cell(&sanitized));
        } else {
            output.extend(sanitized.chars().map(|character| match character {
                '\r' | '\n' | '\t' => ' ',
                other => other,
            }));
        }
    }
    output.push('\n');
}

#[cfg(test)]
#[path = "final_report_test.rs"]
mod tests;
