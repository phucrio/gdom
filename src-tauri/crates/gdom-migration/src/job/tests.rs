use super::*;
use std::str::FromStr;

#[test]
fn persisted_status_values_parse_and_reject_invalid_spelling() {
    assert_eq!(
        JobStatus::from_str("RUNNING_CANARY"),
        Ok(JobStatus::RunningCanary)
    );
    assert_eq!(
        JobStatus::from_str("CANARY_REVIEW"),
        Ok(JobStatus::CanaryReview)
    );
    assert_eq!(
        RootValidationStatus::from_str("VALIDATED"),
        Ok(RootValidationStatus::Validated)
    );
    assert_eq!(
        "running_canary".parse::<JobStatus>(),
        Err(JobError::InvalidJobStatus)
    );
    assert_eq!(
        "VALID".parse::<RootValidationStatus>(),
        Err(JobError::InvalidRootValidationStatus)
    );
}

fn sample_snapshot(id: u128, email: &str, perm: &str) -> AccountSnapshot {
    AccountSnapshot {
        account_id: AccountId::new(id),
        email: email.to_string(),
        display_name: format!("User {id}"),
        permission_id: GooglePermissionId::new(perm),
    }
}

#[test]
fn job_rejects_identical_source_and_target() {
    // Given
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(1, "source@gmail.com", "perm_1");

    // When
    let result = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    );

    // Then
    assert_eq!(result, Err(JobError::SameSourceAndTarget));
}

#[test]
fn job_allows_account_pair_change_while_draft() {
    // Given
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source.clone(),
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("different accounts form a valid job");

    // When
    let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
    let result = job.change_accounts(source, new_target);

    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(job.target_account_id(), AccountId::new(3));
}

#[test]
fn job_rejects_account_pair_change_after_scanning_starts() {
    // Given
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source.clone(),
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("different accounts form a valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");

    // When
    let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
    let result = job.change_accounts(source, new_target);

    // Then
    assert_eq!(result, Err(JobError::AccountPairLocked));
    assert_eq!(job.target_account_id(), AccountId::new(2));
}

#[test]
fn changing_draft_source_clears_validated_roots() {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target.clone(),
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");

    let new_source = sample_snapshot(3, "source3@gmail.com", "perm_3");
    job.change_accounts(new_source, target)
        .expect("draft source can change");

    assert!(job.roots().is_empty());
    assert_eq!(job.source_account_id(), AccountId::new(3));
}

#[test]
fn changing_draft_target_keeps_roots() {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source.clone(),
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");

    let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
    job.change_accounts(source, new_target)
        .expect("draft target can change");

    assert_eq!(job.roots().len(), 1);
}

#[test]
fn start_scanning_requires_a_root_and_rejects_repeat() {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");

    assert_eq!(
        job.start_scanning("2026-09-05T01:00:00Z".to_string()),
        Err(JobError::NoValidatedRoots)
    );

    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");
    job.start_scanning("2026-09-05T01:01:00Z".to_string())
        .expect("in-progress scan can be resumed after a crash");
    assert_eq!(job.status(), JobStatus::Scanning);
}

#[test]
fn pause_and_complete_scanning_follow_legal_transitions() {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");
    job.pause_scanning().expect("scan pauses");
    assert_eq!(job.status(), JobStatus::Paused);
    job.start_scanning("2026-09-05T01:02:00Z".to_string())
        .expect("paused scan resumes");
    job.complete_scanning().expect("scan completes");
    assert_eq!(job.status(), JobStatus::ReadyForReview);
}

#[test]
fn canary_and_bulk_follow_legal_transitions_and_reject_skipping_review() {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");
    job.complete_scanning().expect("scan completes");

    assert_eq!(job.start_bulk(), Err(JobError::IllegalTransition));
    job.start_canary().expect("canary starts");
    assert_eq!(job.status(), JobStatus::RunningCanary);
    job.complete_canary().expect("canary completes");
    assert_eq!(job.status(), JobStatus::CanaryReview);
    job.start_bulk().expect("bulk starts after review");
    job.complete_transfer("2026-09-05T02:00:00Z".to_string(), false)
        .expect("bulk completes");
    assert_eq!(job.status(), JobStatus::Completed);
}

fn job_ready_for_review() -> MigrationJob {
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");
    job.add_root(MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    })
    .expect("root added");
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");
    job.complete_scanning().expect("scan completes");
    job
}

#[test]
fn pause_resume_queue_and_cancel_follow_legal_transfer_transitions() {
    let mut job = job_ready_for_review();
    job.start_canary().expect("canary starts");
    job.pause_transfer().expect("canary pauses");
    assert_eq!(job.status(), JobStatus::Paused);
    job.resume_transfer(true).expect("canary resumes");
    assert_eq!(job.status(), JobStatus::RunningCanary);
    job.complete_canary().expect("canary completes");

    job.enqueue(1).expect("review job can queue");
    assert_eq!(job.status(), JobStatus::Queued);
    assert_eq!(job.queue_position(), Some(1));
    job.resume_transfer(false).expect("queued job resumes bulk");
    assert_eq!(job.status(), JobStatus::Running);
    assert_eq!(job.queue_position(), None);
    job.pause_transfer().expect("bulk pauses");
    job.cancel_job("2026-09-05T03:00:00Z".to_string())
        .expect("paused job cancels");
    assert_eq!(job.status(), JobStatus::Cancelled);
    assert_eq!(job.cancel_job("2026-09-05T03:01:00Z".to_string()), Ok(()));
}

#[test]
fn resuming_after_auth_required_clears_stale_job_error() {
    let mut job = job_ready_for_review();
    job.start_canary().expect("canary starts");
    job.require_auth("invalid authentication credentials".into())
        .expect("auth failure pauses canary");

    job.resume_transfer(true).expect("canary resumes");

    assert_eq!(job.status(), JobStatus::RunningCanary);
    assert_eq!(job.last_error(), None);
}

#[test]
fn auth_halt_accepts_every_resumable_state_and_clears_queue_position() {
    for status in [
        JobStatus::Paused,
        JobStatus::Queued,
        JobStatus::SourceRateLimited,
        JobStatus::WaitingForQuota,
    ] {
        let mut job = job_ready_for_review();
        job.start_canary().expect("canary starts");
        match status {
            JobStatus::Paused => {
                job.pause_transfer().expect("canary pauses");
            }
            JobStatus::Queued => {
                job.pause_transfer().expect("canary pauses");
                job.enqueue(7).expect("paused canary queues");
                assert_eq!(job.queue_position(), Some(7));
            }
            JobStatus::SourceRateLimited => {
                job.pause_sharing_rate_limit("sharing limit".into())
                    .expect("canary hits sharing limit");
            }
            JobStatus::WaitingForQuota => {
                job.wait_for_quota("quota exceeded".into())
                    .expect("canary waits for quota");
            }
            _ => panic!("test state must be resumable"),
        }
        assert_eq!(job.status(), status);

        job.require_auth("reauthentication required".into())
            .expect("auth halt is accepted");

        assert_eq!(job.status(), JobStatus::AuthRequired);
        assert_eq!(job.queue_position(), None);
        assert_eq!(job.last_error(), Some("reauthentication required"));
    }
}

#[test]
fn start_bulk_from_paused_canary_is_illegal() {
    let mut job = job_ready_for_review();
    job.start_canary().expect("canary starts");
    job.pause_transfer().expect("paused");
    assert_eq!(job.start_bulk(), Err(JobError::IllegalTransition));
    job.resume_transfer(true).expect("resume canary");
    assert_eq!(job.status(), JobStatus::RunningCanary);
    assert_eq!(job.start_bulk(), Err(JobError::IllegalTransition));
}

#[test]
fn queued_ready_job_can_start_canary_without_skipping_review() {
    let mut job = job_ready_for_review();
    job.enqueue(1).expect("ready job queues");
    job.start_canary().expect("explicit start after queue");
    assert_eq!(job.status(), JobStatus::RunningCanary);
    assert_eq!(job.queue_position(), None);
    assert_eq!(job.start_bulk(), Err(JobError::IllegalTransition));
}

#[test]
fn enqueue_rejects_active_mutation_jobs() {
    let mut job = job_ready_for_review();
    job.start_canary().expect("canary starts");
    assert_eq!(job.enqueue(1), Err(JobError::IllegalTransition));
    job.pause_transfer().expect("pause");
    job.enqueue(2).expect("paused job can queue");
    assert_eq!(job.queue_position(), Some(2));
}

#[test]
fn job_manages_roots_in_draft_and_locks_in_scanning() {
    // Given
    let source = sample_snapshot(1, "source@gmail.com", "perm_1");
    let target = sample_snapshot(2, "target@gmail.com", "perm_2");
    let mut job = MigrationJob::new(
        JobId::new(100),
        source,
        target,
        "2026-09-05T00:00:00Z".to_string(),
    )
    .expect("valid job");

    let root = MigrationRoot {
        id: RootId::new(501),
        job_id: job.id(),
        root_file_id: "folder_abc".to_string(),
        root_name: "My Folder".to_string(),
        validation_status: RootValidationStatus::Validated,
        created_at: "2026-09-05T00:00:00Z".to_string(),
    };

    // When: add root
    assert_eq!(job.add_root(root.clone()), Ok(()));
    assert_eq!(job.roots().len(), 1);

    // Duplicate root rejected
    assert_eq!(
        job.add_root(root.clone()),
        Err(JobError::DuplicateRoot("folder_abc".to_string()))
    );

    // Lock in scanning
    job.start_scanning("2026-09-05T01:00:00Z".to_string())
        .expect("scan starts");
    assert_eq!(
        job.add_root(MigrationRoot {
            id: RootId::new(502),
            job_id: job.id(),
            root_file_id: "folder_def".to_string(),
            root_name: "Second Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T01:00:00Z".to_string(),
        }),
        Err(JobError::RootsLocked)
    );

    assert_eq!(
        job.remove_root(RootId::new(501)),
        Err(JobError::RootsLocked)
    );
}

#[test]
fn transfer_concurrency_defaults_to_one() {
    assert_eq!(DEFAULT_TRANSFER_CONCURRENCY, 1);
}
