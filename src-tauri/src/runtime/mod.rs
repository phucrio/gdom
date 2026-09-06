//! Checkpointed scan and transfer workers.
//!
//! Long-running Drive work is spawned from [`JobService`] so Tauri commands
//! return the current job immediately. This module owns the Tauri event adapter
//! that publishes progress only after SQLite commits.

use tauri::{AppHandle, Emitter};

use crate::application::job_events::{
    EVENT_CANARY_COMPLETED, EVENT_ITEM_STATE_CHANGED, EVENT_JOB_LIST_CHANGED,
    EVENT_JOB_STATUS_CHANGED, EVENT_MIGRATION_COMPLETED, EVENT_MIGRATION_PROGRESS,
    EVENT_SCAN_PROGRESS, ItemStateChangedPayload, JobEventSink, JobIdPayload, JobRuntimeEvent,
    JobStatusChangedPayload, MigrationProgressPayload, ScanProgressPayload, job_id_key,
};

pub struct TauriJobEventSink {
    app: AppHandle,
}

impl TauriJobEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl JobEventSink for TauriJobEventSink {
    fn emit(&self, event: JobRuntimeEvent) {
        let result = match event {
            JobRuntimeEvent::JobStatusChanged { job_id, status } => self.app.emit(
                EVENT_JOB_STATUS_CHANGED,
                JobStatusChangedPayload {
                    job_id: job_id_key(job_id),
                    status,
                },
            ),
            JobRuntimeEvent::JobListChanged { job_id } => self.app.emit(
                EVENT_JOB_LIST_CHANGED,
                JobIdPayload {
                    job_id: job_id_key(job_id),
                },
            ),
            JobRuntimeEvent::ScanProgress {
                job_id,
                files,
                folders,
                skipped,
            } => self.app.emit(
                EVENT_SCAN_PROGRESS,
                ScanProgressPayload {
                    job_id: job_id_key(job_id),
                    files,
                    folders,
                    skipped,
                },
            ),
            JobRuntimeEvent::MigrationProgress {
                job_id,
                completed,
                total,
                current_path,
            } => self.app.emit(
                EVENT_MIGRATION_PROGRESS,
                MigrationProgressPayload {
                    job_id: job_id_key(job_id),
                    completed,
                    total,
                    current_path,
                },
            ),
            JobRuntimeEvent::ItemStateChanged {
                job_id,
                item_id,
                state,
            } => self.app.emit(
                EVENT_ITEM_STATE_CHANGED,
                ItemStateChangedPayload {
                    job_id: job_id_key(job_id),
                    item_id,
                    state,
                },
            ),
            JobRuntimeEvent::CanaryCompleted { job_id } => self.app.emit(
                EVENT_CANARY_COMPLETED,
                JobIdPayload {
                    job_id: job_id_key(job_id),
                },
            ),
            JobRuntimeEvent::MigrationCompleted { job_id } => self.app.emit(
                EVENT_MIGRATION_COMPLETED,
                JobIdPayload {
                    job_id: job_id_key(job_id),
                },
            ),
        };
        let _ = result;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_CANARY_COMPLETED, EVENT_ITEM_STATE_CHANGED, EVENT_JOB_LIST_CHANGED,
        EVENT_JOB_STATUS_CHANGED, EVENT_MIGRATION_COMPLETED, EVENT_MIGRATION_PROGRESS,
        EVENT_SCAN_PROGRESS,
    };

    #[test]
    fn event_names_match_plan_and_frontend_contract() {
        assert_eq!(EVENT_JOB_STATUS_CHANGED, "job-status-changed");
        assert_eq!(EVENT_JOB_LIST_CHANGED, "job-list-changed");
        assert_eq!(EVENT_SCAN_PROGRESS, "scan-progress");
        assert_eq!(EVENT_MIGRATION_PROGRESS, "migration-progress");
        assert_eq!(EVENT_ITEM_STATE_CHANGED, "item-state-changed");
        assert_eq!(EVENT_CANARY_COMPLETED, "canary-completed");
        assert_eq!(EVENT_MIGRATION_COMPLETED, "migration-completed");
    }
}
