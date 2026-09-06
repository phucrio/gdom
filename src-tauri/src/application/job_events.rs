use crate::domain::job::JobId;

pub const EVENT_JOB_STATUS_CHANGED: &str = "job-status-changed";
pub const EVENT_JOB_LIST_CHANGED: &str = "job-list-changed";
pub const EVENT_SCAN_PROGRESS: &str = "scan-progress";
pub const EVENT_MIGRATION_PROGRESS: &str = "migration-progress";
pub const EVENT_ITEM_STATE_CHANGED: &str = "item-state-changed";
pub const EVENT_CANARY_COMPLETED: &str = "canary-completed";
pub const EVENT_MIGRATION_COMPLETED: &str = "migration-completed";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatusChangedPayload {
    pub job_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressPayload {
    pub job_id: String,
    pub files: u64,
    pub folders: u64,
    pub skipped: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationProgressPayload {
    pub job_id: String,
    pub completed: u64,
    pub total: u64,
    pub current_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStateChangedPayload {
    pub job_id: String,
    pub item_id: String,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobIdPayload {
    pub job_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobRuntimeEvent {
    JobStatusChanged {
        job_id: JobId,
        status: String,
    },
    JobListChanged {
        job_id: JobId,
    },
    ScanProgress {
        job_id: JobId,
        files: u64,
        folders: u64,
        skipped: u64,
    },
    MigrationProgress {
        job_id: JobId,
        completed: u64,
        total: u64,
        current_path: Option<String>,
    },
    ItemStateChanged {
        job_id: JobId,
        item_id: String,
        state: String,
    },
    CanaryCompleted {
        job_id: JobId,
    },
    MigrationCompleted {
        job_id: JobId,
    },
}

pub trait JobEventSink: Send + Sync {
    fn emit(&self, event: JobRuntimeEvent);
}

pub struct NoopJobEventSink;

impl JobEventSink for NoopJobEventSink {
    fn emit(&self, _event: JobRuntimeEvent) {}
}

pub struct RecordingJobEventSink {
    events: std::sync::Mutex<Vec<JobRuntimeEvent>>,
}

impl RecordingJobEventSink {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn snapshot(&self) -> Vec<JobRuntimeEvent> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl Default for RecordingJobEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl JobEventSink for RecordingJobEventSink {
    fn emit(&self, event: JobRuntimeEvent) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
    }
}

pub fn job_id_key(job_id: JobId) -> String {
    job_id.to_string()
}
