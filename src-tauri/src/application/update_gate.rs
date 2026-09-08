use std::sync::Arc;

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use super::JobServiceError;

#[derive(Clone, Default)]
pub(super) struct UpdateGate(Arc<RwLock<()>>);

pub struct UpdateInstallationGuard {
    _exclusive: OwnedRwLockWriteGuard<()>,
}

impl UpdateGate {
    pub(super) fn begin_work(&self) -> Result<OwnedRwLockReadGuard<()>, JobServiceError> {
        Arc::clone(&self.0)
            .try_read_owned()
            .map_err(|_| JobServiceError::UpdateInstallationInProgress)
    }

    pub(super) fn begin_installation(&self) -> Result<UpdateInstallationGuard, JobServiceError> {
        Arc::clone(&self.0)
            .try_write_owned()
            .map(|exclusive| UpdateInstallationGuard {
                _exclusive: exclusive,
            })
            .map_err(|_| JobServiceError::UpdateInstallationBusy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_waits_for_all_workers_across_clones() {
        let gate = UpdateGate::default();
        let clone = gate.clone();
        let first = gate.begin_work().expect("first worker");
        let second = clone.begin_work().expect("second worker");
        drop(first);
        assert!(matches!(
            gate.begin_installation(),
            Err(JobServiceError::UpdateInstallationBusy)
        ));
        drop(second);
        assert!(gate.begin_installation().is_ok());
    }

    #[test]
    fn installation_guard_blocks_work_until_dropped() {
        let gate = UpdateGate::default();
        let installation = gate.begin_installation().expect("idle installation");
        assert!(matches!(
            gate.clone().begin_work(),
            Err(JobServiceError::UpdateInstallationInProgress)
        ));
        assert!(matches!(
            gate.begin_installation(),
            Err(JobServiceError::UpdateInstallationBusy)
        ));
        drop(installation);
        assert!(gate.begin_work().is_ok());
    }
}
