use crate::application::drive_tree::StorageQuota;
use crate::application::item_store::ItemAggregates;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightSummary {
    pub total_items: u64,
    pub eligible_items: u64,
    pub eligible_files: u64,
    pub eligible_folders: u64,
    pub skipped_already_owned_by_target: u64,
    pub skipped_not_owned_by_source: u64,
    pub skipped_shared_drive: u64,
    pub skipped_shortcuts: u64,
    pub skipped_trashed: u64,
    pub skipped_ineligible: u64,
    pub estimated_quota_bytes: u64,
    pub target_usage_bytes: u64,
    pub target_limit_bytes: Option<u64>,
    pub target_remaining_bytes: Option<u64>,
    pub quota_warning: bool,
}

impl PreflightSummary {
    pub fn from_aggregates(aggregates: &ItemAggregates, quota: &StorageQuota) -> Self {
        let remaining = quota
            .limit_bytes
            .map(|limit| limit.saturating_sub(quota.usage_bytes));
        let quota_warning = remaining.is_some_and(|left| aggregates.estimated_quota_bytes > left);
        Self {
            total_items: aggregates.total,
            eligible_items: aggregates.eligible,
            eligible_files: aggregates.eligible_files,
            eligible_folders: aggregates.eligible_folders,
            skipped_already_owned_by_target: aggregates.skipped_already_owned_by_target,
            skipped_not_owned_by_source: aggregates.skipped_not_owned_by_source,
            skipped_shared_drive: aggregates.skipped_shared_drive,
            skipped_shortcuts: aggregates.skipped_shortcuts,
            skipped_trashed: aggregates.skipped_trashed,
            skipped_ineligible: aggregates.skipped_ineligible,
            estimated_quota_bytes: aggregates.estimated_quota_bytes,
            target_usage_bytes: quota.usage_bytes,
            target_limit_bytes: quota.limit_bytes,
            target_remaining_bytes: remaining,
            quota_warning,
        }
    }

    pub fn skipped_total(&self) -> u64 {
        self.skipped_already_owned_by_target
            + self.skipped_not_owned_by_source
            + self.skipped_shared_drive
            + self.skipped_shortcuts
            + self.skipped_trashed
            + self.skipped_ineligible
    }

    pub fn render_report(
        &self,
        job_id: &str,
        source_email: &str,
        target_email: &str,
        roots: &[String],
    ) -> String {
        let limit = self
            .target_limit_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unlimited".to_string());
        let remaining = self
            .target_remaining_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unlimited".to_string());
        let roots_list = if roots.is_empty() {
            "(none)".to_string()
        } else {
            roots.join(", ")
        };
        format!(
            "GDOM dry-run preflight\n\
             Job: {job_id}\n\
             Source: {source_email}\n\
             Target: {target_email}\n\
             Root folders: {roots_list}\n\
             Total items: {}\n\
             Eligible items: {}\n\
             Eligible files: {}\n\
             Eligible folders: {}\n\
             Already target-owned: {}\n\
             Not owned by source: {}\n\
             Shared Drive: {}\n\
             Shortcuts: {}\n\
             Trashed or missing: {}\n\
             Other ineligible: {}\n\
             Estimated quota bytes: {}\n\
             Target usage bytes: {}\n\
             Target limit bytes: {limit}\n\
             Target remaining bytes: {remaining}\n\
             Quota sufficient: {}\n",
            self.total_items,
            self.eligible_items,
            self.eligible_files,
            self.eligible_folders,
            self.skipped_already_owned_by_target,
            self.skipped_not_owned_by_source,
            self.skipped_shared_drive,
            self.skipped_shortcuts,
            self.skipped_trashed,
            self.skipped_ineligible,
            self.estimated_quota_bytes,
            self.target_usage_bytes,
            if self.quota_warning { "no" } else { "yes" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_warning_when_estimated_exceeds_remaining() {
        let aggregates = ItemAggregates {
            total: 2,
            eligible: 2,
            estimated_quota_bytes: 80,
            ..ItemAggregates::default()
        };
        let quota = StorageQuota {
            limit_bytes: Some(100),
            usage_bytes: 40,
        };
        let summary = PreflightSummary::from_aggregates(&aggregates, &quota);
        assert!(summary.quota_warning);
        assert_eq!(summary.target_remaining_bytes, Some(60));
    }

    #[test]
    fn unlimited_quota_is_not_a_warning() {
        let aggregates = ItemAggregates {
            estimated_quota_bytes: 999_999,
            ..ItemAggregates::default()
        };
        let quota = StorageQuota {
            limit_bytes: None,
            usage_bytes: 1,
        };
        let summary = PreflightSummary::from_aggregates(&aggregates, &quota);
        assert!(!summary.quota_warning);
        assert!(summary.target_remaining_bytes.is_none());
    }

    #[test]
    fn report_contains_counts_and_quota_not_tokens() {
        let summary = PreflightSummary {
            total_items: 4,
            eligible_items: 2,
            eligible_files: 1,
            eligible_folders: 1,
            skipped_already_owned_by_target: 1,
            skipped_not_owned_by_source: 0,
            skipped_shared_drive: 0,
            skipped_shortcuts: 1,
            skipped_trashed: 0,
            skipped_ineligible: 0,
            estimated_quota_bytes: 50,
            target_usage_bytes: 10,
            target_limit_bytes: Some(1000),
            target_remaining_bytes: Some(990),
            quota_warning: false,
        };
        let report = summary.render_report(
            "123",
            "source@gmail.com",
            "target@gmail.com",
            &["Root".into()],
        );
        assert!(report.contains("Eligible items: 2"));
        assert!(report.contains("Target remaining bytes: 990"));
        assert!(report.contains("Quota sufficient: yes"));
        assert!(!report.to_ascii_lowercase().contains("bearer"));
        assert!(!report.contains("access_token"));
        assert!(!report.contains("refresh_token"));
    }
}
