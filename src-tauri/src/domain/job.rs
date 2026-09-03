use super::AccountId;

pub const DEFAULT_TRANSFER_CONCURRENCY: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobStatus {
    Draft,
    Scanning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AccountPair {
    source: AccountId,
    target: AccountId,
}

impl AccountPair {
    const fn new(source: AccountId, target: AccountId) -> Result<Self, JobError> {
        if source.0 == target.0 {
            return Err(JobError::SameSourceAndTarget);
        }

        Ok(Self { source, target })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobError {
    SameSourceAndTarget,
    AccountPairLocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationJob {
    accounts: AccountPair,
    status: JobStatus,
}

impl MigrationJob {
    pub const fn new(
        source_account_id: AccountId,
        target_account_id: AccountId,
    ) -> Result<Self, JobError> {
        match AccountPair::new(source_account_id, target_account_id) {
            Ok(accounts) => Ok(Self {
                accounts,
                status: JobStatus::Draft,
            }),
            Err(error) => Err(error),
        }
    }

    pub const fn change_accounts(
        &mut self,
        source_account_id: AccountId,
        target_account_id: AccountId,
    ) -> Result<(), JobError> {
        match self.status {
            JobStatus::Draft => match AccountPair::new(source_account_id, target_account_id) {
                Ok(accounts) => {
                    self.accounts = accounts;
                    Ok(())
                }
                Err(error) => Err(error),
            },
            JobStatus::Scanning => Err(JobError::AccountPairLocked),
        }
    }

    pub const fn start_scanning(&mut self) {
        self.status = JobStatus::Scanning;
    }

    pub const fn target_account_id(&self) -> AccountId {
        self.accounts.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_rejects_identical_source_and_target() {
        // Given
        let account_id = AccountId::new(1);

        // When
        let result = MigrationJob::new(account_id, account_id);

        // Then
        assert_eq!(result, Err(JobError::SameSourceAndTarget));
    }

    #[test]
    fn job_allows_account_pair_change_while_draft() {
        // Given
        let mut job = MigrationJob::new(AccountId::new(1), AccountId::new(2))
            .expect("different accounts form a valid job");

        // When
        let result = job.change_accounts(AccountId::new(1), AccountId::new(3));

        // Then
        assert_eq!(result, Ok(()));
        assert_eq!(job.target_account_id(), AccountId::new(3));
    }

    #[test]
    fn job_rejects_account_pair_change_after_scanning_starts() {
        // Given
        let mut job = MigrationJob::new(AccountId::new(1), AccountId::new(2))
            .expect("different accounts form a valid job");
        job.start_scanning();

        // When
        let result = job.change_accounts(AccountId::new(1), AccountId::new(3));

        // Then
        assert_eq!(result, Err(JobError::AccountPairLocked));
        assert_eq!(job.target_account_id(), AccountId::new(2));
    }

    #[test]
    fn transfer_concurrency_defaults_to_one() {
        // Given
        let expected_safe_default = 1;

        // When
        let actual = DEFAULT_TRANSFER_CONCURRENCY;

        // Then
        assert_eq!(actual, expected_safe_default);
    }
}
