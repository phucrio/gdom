use super::{
    gate::{
        Fixture, LiveResult, Manifest, external_existing_path, selected_fixture, validate_manifest,
    },
    guarded_drive::GuardedDrive,
};
use crate::{
    application::{AccountTokenProvider, DrivePort, JobService, RefreshTokenStore},
    domain::{
        AccountId, AuthStatus,
        job::{JobId, JobStatus},
    },
    infrastructure::{
        SqliteJobStore, account_store::SqliteAccountStore, google_drive::GoogleDriveClient,
        google_token::DynamicGoogleTokenClient, secrets::WindowsCredentialStore,
    },
    state::OAuthConfig,
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

type TestJobService = JobService<SqliteAccountStore, SqliteJobStore>;

pub async fn run(diagnostics: &mut super::diagnostics::Diagnostics) -> LiveResult<()> {
    diagnostics.stage =
        "Commit or remove source changes and untracked files before recording live evidence";
    let revision = super::gate::source_revision()?;
    diagnostics.stage = "Set GDOM_LIVE_MANIFEST to a valid local manifest outside Git";
    let manifest_path = external_existing_path(Path::new(&std::env::var("GDOM_LIVE_MANIFEST")?))?;
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(manifest_path)?)?;
    validate_manifest(&manifest)?;
    diagnostics.stage =
        "Create an output directory outside Git and confirm the selected fixture root";
    let output = external_existing_path(&manifest.output_directory)?;
    if !output.is_dir() {
        return Err("output must be an existing directory".into());
    }
    diagnostics.output_directory = Some(output.clone());
    let selected = std::env::var("GDOM_LIVE_TARGET").ok();
    let fixtures: Vec<&Fixture> = match selected.as_deref() {
        None => manifest.fixtures.iter().collect(),
        Some(target) => vec![selected_fixture(
            &manifest,
            target,
            std::env::var("GDOM_LIVE_CONFIRM_ROOT").ok().as_deref(),
        )?],
    };
    let database = external_existing_path(&manifest.database)?;
    diagnostics.stage =
        "Use a dedicated database containing exactly the three connected manifest accounts";
    let store = Arc::new(SqliteAccountStore::open(&database).await?);
    let accounts = store.list_all().await?;
    if accounts.len() != 3 {
        return Err("use a dedicated database containing exactly A, B, C".into());
    }
    for specification in &manifest.accounts {
        let account = accounts
            .iter()
            .find(|account| account.id().value().to_string() == specification.account_id)
            .ok_or("account missing")?;
        if account.auth_status() != AuthStatus::Connected
            || !account.email().eq_ignore_ascii_case(&specification.email)
        {
            return Err("account identity mismatch".into());
        }
    }
    if accounts
        .iter()
        .map(|account| account.google_permission_id())
        .collect::<HashSet<_>>()
        .len()
        != 3
    {
        return Err("accounts must have distinct Google identities".into());
    }
    diagnostics.stage = "Check local OAuth setup and reconnect the dedicated accounts through GDOM";
    let credentials = Arc::new(WindowsCredentialStore::new()?);
    let client_id = store.get_setting("oauth.client_id").await?;
    let configuration = OAuthConfig::resolve(
        client_id.as_deref(),
        credentials.load_oauth_secret()?,
        |key| std::env::var(key),
    )
    .0;
    let refresh = Arc::new(DynamicGoogleTokenClient::new(Arc::new(
        tokio::sync::RwLock::new(Some(configuration)),
    )));
    let provider = Arc::new(AccountTokenProvider::new(
        refresh,
        credentials,
        Arc::clone(&store),
    ));
    let client = GoogleDriveClient::new()?;
    // Validate every account against Google, not just an editable manifest or stale database row.
    for specification in &manifest.accounts {
        let account_id = AccountId::new(specification.account_id.parse()?);
        let token = provider.get_access_token(account_id).await?;
        let identity = client.account_identity(&token).await?;
        let account = store
            .find_by_id(account_id)
            .await?
            .ok_or("account missing")?;
        if identity.permission_id() != account.google_permission_id()
            || !identity.email().eq_ignore_ascii_case(&specification.email)
        {
            return Err("live identity mismatch".into());
        }
    }
    let jobs = Arc::new(SqliteJobStore::new(store.pool().clone()));
    for fixture in fixtures {
        diagnostics.stage = "Confirm the fixture exists, belongs exclusively to A, and contains only the listed root, subfolder and files";
        let source_id = AccountId::new(manifest.accounts[0].account_id.parse()?);
        let target_specification = manifest
            .accounts
            .iter()
            .find(|account| account.label == fixture.target)
            .ok_or("target missing")?;
        let target_id = AccountId::new(target_specification.account_id.parse()?);
        let source = store.find_by_id(source_id).await?.ok_or("source missing")?;
        let target = store.find_by_id(target_id).await?.ok_or("target missing")?;
        let guard = Arc::new(GuardedDrive {
            client: client.clone(),
            source_token: provider.get_access_token(source_id).await?,
            target_token: provider.get_access_token(target_id).await?,
            target_email: target.email().into(),
            allowed_ids: fixture.item_ids.iter().cloned().collect(),
            mutations_enabled: AtomicBool::new(false),
            pending_seen: AtomicBool::new(false),
            accept_seen: AtomicBool::new(false),
            target_read_seen: AtomicBool::new(false),
        });
        let service = JobService::new(
            Arc::clone(&store),
            Arc::clone(&jobs),
            guard.clone() as Arc<dyn DrivePort>,
            Arc::clone(&provider),
        );
        if service.list_jobs().await?.iter().any(|job| {
            matches!(
                job.status(),
                JobStatus::Scanning
                    | JobStatus::Running
                    | JobStatus::RunningCanary
                    | JobStatus::Pausing
                    | JobStatus::Cancelling
            )
        }) {
            return Err("another job is active; close the app before running".into());
        }
        let mut parents = HashMap::new();
        let mut folders = 0;
        for item_id in &fixture.item_ids {
            let snapshot = client.get_file(&guard.source_token, item_id).await?;
            if snapshot.trashed
                || snapshot.drive_id.is_some()
                || snapshot.owners.len() != 1
                || snapshot.owners[0].permission_id != *source.google_permission_id()
            {
                return Err("fixture must be untrashed and exclusively owned by A".into());
            }
            if snapshot.mime_type == "application/vnd.google-apps.folder" {
                folders += 1;
            }
            parents.insert(item_id, snapshot.parents);
        }
        if folders < 2 || folders == fixture.item_ids.len() {
            return Err("fixture requires root, subfolder and file".into());
        }
        let job = service.create_job(source_id, target_id).await?;
        if fixture.item_ids.len() > job.canary_size() {
            return Err("fixture exceeds runtime canary size".into());
        }
        service.add_root(job.id(), &fixture.root_id).await?;
        diagnostics.stage =
            "Inspect the local scan job and make the manifest match every eligible fixture item";
        service.start_scan(job.id()).await?;
        wait_for_worker(&service, job.id()).await?;
        let scanned = service.list_job_items(job.id(), None, 1).await?;
        if service.get_job(job.id()).await?.status() != JobStatus::ReadyForReview
            || scanned
                .items
                .iter()
                .any(|item| item.state != crate::domain::item::ItemState::Eligible)
            || scanned
                .items
                .iter()
                .map(|item| &item.file_id)
                .collect::<HashSet<_>>()
                != fixture.item_ids.iter().collect()
        {
            return Err("scan does not exactly match fixture manifest".into());
        }
        if selected.is_none() {
            std::fs::write(
                output.join(format!("preflight-{}.json", fixture.target)),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"target": fixture.target, "items": scanned.total, "identity": "PASS", "scan": "PASS", "mutations": false}),
                )?,
            )?;
            continue;
        }
        diagnostics.stage = "Inspect the retained transfer job before retrying; confirm source and target remain connected";
        if super::gate::source_revision()? != revision {
            return Err("source changed during preflight".into());
        }
        guard.mutations_enabled.store(true, Ordering::SeqCst);
        service.run_auto_mutation_if_ready(job.id()).await?;
        wait_for_worker(&service, job.id()).await?;
        guard.mutations_enabled.store(false, Ordering::SeqCst);
        let completed = service.get_job(job.id()).await?;
        if completed.status() != JobStatus::Completed {
            return Err("canary incomplete; retain local job and inspect before retry".into());
        }
        if !guard.pending_seen.load(Ordering::SeqCst)
            || !guard.accept_seen.load(Ordering::SeqCst)
            || !guard.target_read_seen.load(Ordering::SeqCst)
        {
            return Err("token routing evidence incomplete".into());
        }
        diagnostics.stage =
            "Check each transferred fixture owner and parents using the selected target";
        for item_id in &fixture.item_ids {
            let snapshot = client.get_file(&guard.target_token, item_id).await?;
            if snapshot.owners.len() != 1
                || snapshot.owners[0].permission_id != *target.google_permission_id()
                || parents.get(item_id) != Some(&snapshot.parents)
            {
                return Err("owner or parents verification failed".into());
            }
        }
        diagnostics.stage =
            "Check the local output directory and final report; retain the completed job";
        if super::gate::source_revision()? != revision {
            return Err("source changed during transfer".into());
        }
        for extension in ["txt", "csv"] {
            let destination = output.join(format!("final-{}.{}", fixture.target, extension));
            service
                .export_final_report(job.id(), destination.to_str().ok_or("invalid report path")?)
                .await?;
        }
        std::fs::write(
            output.join(format!("evidence-{}.json", fixture.target)),
            serde_json::to_vec_pretty(
                &serde_json::json!({"target":fixture.target,"items":scanned.total,"owner":"PASS","parents":"PASS","sourcePendingTargetAcceptVerify":"PASS","notificationReceipt":"OPERATOR_REQUIRED","duplicateConnection":"OPERATOR_REQUIRED","build":env!("CARGO_PKG_VERSION"),"revision":revision,"timestamp":crate::application::time::iso_now()}),
            )?,
        )?;
    }
    Ok(())
}

async fn wait_for_worker(service: &TestJobService, job_id: JobId) -> LiveResult<()> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
    while tokio::time::Instant::now() < deadline {
        if !matches!(
            service.get_job(job_id).await?.status(),
            JobStatus::Scanning
                | JobStatus::RunningCanary
                | JobStatus::Running
                | JobStatus::Pausing
                | JobStatus::Cancelling
        ) {
            service.await_idle(job_id).await;
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("worker timed out; retained job requires inspection before retry".into())
}
