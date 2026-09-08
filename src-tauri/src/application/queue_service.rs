use super::*;

impl<A, J> JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    pub async fn queue_job(
        &self,
        job_id: JobId,
        position: Option<i64>,
    ) -> Result<MigrationJob, JobServiceError> {
        self.change_queue(job_id, position, false).await
    }

    pub async fn reorder_queued_job(
        &self,
        job_id: JobId,
        position: i64,
    ) -> Result<MigrationJob, JobServiceError> {
        self.change_queue(job_id, Some(position), true).await
    }

    async fn change_queue(
        &self,
        job_id: JobId,
        position: Option<i64>,
        require_queued: bool,
    ) -> Result<MigrationJob, JobServiceError> {
        let expected = self.job_store.list_jobs().await?;
        let mut target = expected
            .iter()
            .find(|job| job.id() == job_id)
            .cloned()
            .ok_or(JobServiceError::JobNotFound(job_id))?;
        if require_queued && target.status() != JobStatus::Queued {
            return Err(JobServiceError::IllegalTransition);
        }
        let mut queued: Vec<_> = expected
            .iter()
            .filter(|job| job.status() == JobStatus::Queued && job.id() != job_id)
            .cloned()
            .collect();
        queued.sort_by_key(|job| (job.queue_position().unwrap_or(i64::MAX), job.id().value()));
        let insert_at = match position {
            Some(position) => usize::try_from(position)
                .ok()
                .and_then(|value| value.checked_sub(1))
                .filter(|index| *index <= queued.len())
                .ok_or(JobServiceError::IllegalTransition)?,
            None => queued.len(),
        };
        target.enqueue(
            i64::try_from(insert_at + 1).map_err(|_| JobServiceError::IllegalTransition)?,
        )?;
        queued.insert(insert_at, target);
        self.persist_queue(&expected, queued).await?;
        self.get_job(job_id).await
    }

    pub async fn remove_queued_job(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let expected = self.job_store.list_jobs().await?;
        let mut target = expected
            .iter()
            .find(|job| job.id() == job_id)
            .cloned()
            .ok_or(JobServiceError::JobNotFound(job_id))?;
        if target.status() != JobStatus::Queued {
            return Err(JobServiceError::IllegalTransition);
        }
        let latest = self.job_store.latest_job_event(job_id).await?;
        let cohort = self.job_store.list_canary_cohort(job_id).await?;
        let scan_checkpoints = self.job_store.list_scan_checkpoints(job_id).await?;
        let queued_from = self.job_store.queued_from_status(job_id).await?;
        let restored = if queued_from == Some(JobStatus::ReadyForReview) {
            JobStatus::ReadyForReview
        } else if queued_from == Some(JobStatus::Paused) {
            JobStatus::Paused
        } else if queued_from == Some(JobStatus::CanaryReview)
            || latest.as_ref().is_some_and(|event| {
                event.new_state.as_deref() == Some(JobStatus::CanaryReview.as_str())
            })
        {
            JobStatus::CanaryReview
        } else if !scan_checkpoints.is_empty()
            || latest
                .as_ref()
                .is_some_and(|event| event.new_state.as_deref() == Some(JobStatus::Paused.as_str()))
        {
            JobStatus::Paused
        } else if cohort.is_empty() && !self.looks_like_transfer_pause(&target).await? {
            JobStatus::ReadyForReview
        } else {
            JobStatus::Paused
        };
        target.remove_from_queue(restored)?;
        let mut queued: Vec<_> = expected
            .iter()
            .filter(|job| job.status() == JobStatus::Queued && job.id() != job_id)
            .cloned()
            .collect();
        queued.sort_by_key(|job| (job.queue_position().unwrap_or(i64::MAX), job.id().value()));
        queued.push(target);
        self.persist_queue(&expected, queued).await?;
        self.get_job(job_id).await
    }

    async fn persist_queue(
        &self,
        expected: &[MigrationJob],
        jobs: Vec<MigrationJob>,
    ) -> Result<(), JobServiceError> {
        let mut changes = Vec::with_capacity(jobs.len());
        for (index, mut job) in jobs.into_iter().enumerate() {
            if job.status() == JobStatus::Queued {
                job.set_queue_position(Some(
                    i64::try_from(index + 1).map_err(|_| JobServiceError::IllegalTransition)?,
                ));
            }
            let previous = expected
                .iter()
                .find(|original| original.id() == job.id())
                .ok_or(JobServiceError::JobNotFound(job.id()))?;
            let event = Self::status_event(&job, Some(previous.status().as_str()), "QUEUE_UPDATED");
            changes.push((job, event));
        }
        self.job_store
            .persist_queue_changes(expected, &changes)
            .await?;
        for (job, _) in changes {
            self.emit_status(&job);
        }
        Ok(())
    }
}
