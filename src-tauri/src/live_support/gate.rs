use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub type LiveResult<T> = Result<T, Box<dyn std::error::Error>>;
pub const CANARY_LIMIT: usize = 5;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub database: PathBuf,
    pub output_directory: PathBuf,
    pub accounts: [TestAccount; 3],
    pub fixtures: [Fixture; 2],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestAccount {
    pub label: String,
    pub account_id: String,
    pub email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub target: String,
    pub root_id: String,
    pub item_ids: Vec<String>,
}

pub fn check_environment(enabled: Option<&str>, ci: bool) -> LiveResult<()> {
    if ci || enabled != Some("1") {
        return Err("live harness requires explicit opt-in and refuses CI".into());
    }
    Ok(())
}

pub fn validate_manifest(manifest: &Manifest) -> LiveResult<()> {
    let mut identities = HashSet::new();
    let mut emails = HashSet::new();
    for (account, expected_label) in manifest.accounts.iter().zip(["A", "B", "C"]) {
        let email = account.email.to_ascii_lowercase();
        let Some((local, domain)) = email.split_once('@') else {
            return Err("invalid personal Gmail identity".into());
        };
        if account.label != expected_label
            || local.is_empty()
            || !matches!(domain, "gmail.com" | "googlemail.com")
            || account.account_id.parse::<u128>().is_err()
            || !identities.insert(account.account_id.parse::<u128>()?)
            || !emails.insert(format!("{}@gmail.com", local.replace('.', "")))
        {
            return Err("manifest requires distinct personal Gmail accounts A, B, C".into());
        }
    }
    let mut all_items = HashSet::new();
    for (fixture, expected_target) in manifest.fixtures.iter().zip(["B", "C"]) {
        if fixture.target != expected_target
            || !(3..=CANARY_LIMIT).contains(&fixture.item_ids.len())
            || !fixture.item_ids.contains(&fixture.root_id)
        {
            return Err(
                "manifest requires two bounded root/subfolder/file fixtures for B and C".into(),
            );
        }
        for item_id in &fixture.item_ids {
            if item_id.is_empty()
                || !item_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                || !all_items.insert(item_id)
            {
                return Err("fixture IDs must be valid, unique, and disjoint".into());
            }
        }
    }
    Ok(())
}

pub fn external_existing_path(path: &Path) -> LiveResult<PathBuf> {
    let resolved = path.canonicalize()?;
    let directory = if resolved.is_dir() {
        resolved.as_path()
    } else {
        resolved.parent().ok_or("path has no parent directory")?
    };
    let result = create_repository_git_command(directory)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()?;
    if result.status.success() {
        return Err(
            "live manifest, database, and output must be outside every Git checkout".into(),
        );
    }
    if result.status.code() != Some(128)
        || !String::from_utf8_lossy(&result.stderr).contains("not a git repository")
    {
        return Err("cannot establish that the local path is outside Git".into());
    }
    Ok(resolved)
}

pub fn selected_fixture<'a>(
    manifest: &'a Manifest,
    target: &str,
    confirmation: Option<&str>,
) -> LiveResult<&'a Fixture> {
    let fixture = manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.target == target)
        .ok_or("select B or C only")?;
    if confirmation != Some(fixture.root_id.as_str()) {
        return Err("explicit confirmation must equal this fixture root ID".into());
    }
    Ok(fixture)
}

pub fn source_revision() -> LiveResult<String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository missing")?;
    let status = create_repository_git_command(repository)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err("live evidence requires a clean checkout including untracked files".into());
    }
    let output = create_repository_git_command(repository)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let revision = String::from_utf8(output.stdout)?.trim().to_owned();
    if !output.status.success()
        || revision.len() != 40
        || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("source commit could not be resolved".into());
    }
    Ok(revision)
}

pub fn create_repository_git_command(directory: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(directory).env("LC_ALL", "C");
    // Caller Git routing/configuration must not redirect fixture checks or source evidence.
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(name);
        }
    }
    command
}
