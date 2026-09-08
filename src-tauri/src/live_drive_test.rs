#[path = "live_support/mod.rs"]
mod live_support;
use live_support::gate::{Manifest, check_environment, selected_fixture, validate_manifest};

#[tokio::test]
#[ignore = "requires dedicated Gmail fixtures and explicit local authorization; never run in CI"]
async fn dedicated_gmail_canary() {
    let mut diagnostics = live_support::diagnostics::Diagnostics {
        stage: "Enable the live opt-in on Windows outside CI",
        output_directory: None,
    };
    let outcome = async {
        check_environment(
            std::env::var("GDOM_LIVE_DRIVE_TESTS").ok().as_deref(),
            ["CI", "GITHUB_ACTIONS", "TF_BUILD", "BUILD_BUILDID"]
                .iter()
                .any(|key| std::env::var_os(key).is_some()),
        )?;
        #[cfg(target_os = "windows")]
        live_support::run::run(&mut diagnostics).await?;
        #[cfg(not(target_os = "windows"))]
        return Err::<(), Box<dyn std::error::Error>>("live harness requires Windows".into());
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    if let Err(error) = outcome {
        let recorded = diagnostics.record_failure(error.as_ref());
        panic!(
            "live canary failed: {}. Local sanitized failure.json written: {}. See docs/testing.md; no live success is recorded.",
            diagnostics.stage, recorded
        );
    }
}

fn manifest() -> Manifest {
    serde_json::from_str(
        r#"{"database":"outside.db","output_directory":"outside", "accounts":[
        {"label":"A","account_id":"1","email":"testa@gmail.com"},
        {"label":"B","account_id":"2","email":"testb@gmail.com"},
        {"label":"C","account_id":"3","email":"testc@googlemail.com"}],
        "fixtures":[{"target":"B","root_id":"rootB","item_ids":["rootB","subB","fileB"]},
        {"target":"C","root_id":"rootC","item_ids":["rootC","subC","fileC"]}]}"#,
    )
    .unwrap()
}

#[test]
fn disabled_or_ci_environment_blocks_before_loading_any_manifest() {
    for enabled in [None, Some("0"), Some("true")] {
        assert!(check_environment(enabled, false).is_err());
    }
    assert!(check_environment(Some("1"), true).is_err());
    assert!(check_environment(Some("1"), false).is_ok());
}
#[test]
fn manifest_rejects_shared_accounts_or_out_of_scope_items() {
    assert!(validate_manifest(&manifest()).is_ok());
    let mut duplicate = manifest();
    duplicate.accounts[2].account_id = "2".into();
    assert!(validate_manifest(&duplicate).is_err());
    let mut shared = manifest();
    shared.fixtures[1].item_ids[1] = "subB".into();
    assert!(validate_manifest(&shared).is_err());
    let mut oversized = manifest();
    oversized.fixtures[0]
        .item_ids
        .extend(["extra1".into(), "extra2".into(), "extra3".into()]);
    assert!(validate_manifest(&oversized).is_err());
    let mut workspace = manifest();
    workspace.accounts[0].email = "a@example.com".into();
    assert!(validate_manifest(&workspace).is_err());
}
#[test]
fn authorization_is_specific_to_one_fixture() {
    let manifest = manifest();
    assert!(selected_fixture(&manifest, "B", None).is_err());
    assert!(selected_fixture(&manifest, "B", Some("rootC")).is_err());
    assert!(selected_fixture(&manifest, "B", Some("rootB")).is_ok());
}

#[test]
fn local_paths_reject_other_git_checkouts() -> live_support::gate::LiveResult<()> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("gdom-path-gate-{}-{unique}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let checkout = directory.join("checkout");
    let result = std::process::Command::new("git")
        .arg("init")
        .arg(&checkout)
        .output()?;
    assert!(result.status.success());
    let manifest = checkout.join("manifest.json");
    std::fs::write(&manifest, "{}")?;
    assert!(live_support::gate::external_existing_path(&checkout).is_err());
    assert!(live_support::gate::external_existing_path(&manifest).is_err());
    assert_eq!(
        live_support::gate::external_existing_path(&directory)?,
        directory.canonicalize()?
    );
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn repository_probe_ignores_inherited_git_routing() -> live_support::gate::LiveResult<()> {
    if std::env::var_os("GDOM_GIT_PROBE_CHILD").is_some() {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("repository missing")?;
        let output = live_support::gate::create_repository_git_command(repository)
            .args(["rev-parse", "--show-toplevel"])
            .output()?;
        assert!(
            output.status.success(),
            "Git must inspect the requested repository"
        );
        let actual = String::from_utf8(output.stdout)?;
        assert_eq!(
            std::path::Path::new(actual.trim()).canonicalize()?,
            repository.canonicalize()?
        );
        return Ok(());
    }
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "live_drive_test::repository_probe_ignores_inherited_git_routing",
        ])
        .env("GDOM_GIT_PROBE_CHILD", "1")
        .env("GIT_DIR", "nonexistent-gdom-git-directory")
        .env("GIT_WORK_TREE", std::env::temp_dir())
        .env("GIT_INDEX_FILE", "nonexistent-gdom-git-index")
        .output()?;
    assert!(
        output.status.success(),
        "Git routing regression child failed"
    );
    Ok(())
}
