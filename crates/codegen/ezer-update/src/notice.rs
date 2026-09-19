//! One-shot "update available" notice persisted under the ezer home.
//!
//! Path: `{EZER_HOME}/update-notice.json` (default `~/.ezer/update-notice.json`).
//! The file records the last upstream version the user was told about so
//! startup does not repeat the same notice every launch.
//!
//! Version checks use this fork's GitHub releases (`iwen-conf/ezer`, or
//! `$EZER_UPSTREAM_REPO`). They never query xAI / ezer.com / x.ai channels.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default GitHub `owner/repo` for the ezer fork.
pub const DEFAULT_EZER_UPSTREAM_REPO: &str = "iwen-conf/ezer";

/// Filename under the ezer home (`~/.ezer` / `$EZER_HOME`).
pub const UPDATE_NOTICE_FILENAME: &str = "update-notice.json";

/// On-disk payload for [`UPDATE_NOTICE_FILENAME`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNoticeState {
    /// Last upstream version we already showed a notice for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified_version: Option<String>,
}

/// `{home}/update-notice.json`.
pub fn update_notice_path(home: &Path) -> PathBuf {
    home.join(UPDATE_NOTICE_FILENAME)
}

/// Load the persisted once-flag. Missing or unreadable files mean "never notified".
pub fn load_update_notice(home: &Path) -> UpdateNoticeState {
    let path = update_notice_path(home);
    let Ok(bytes) = std::fs::read(&path) else {
        return UpdateNoticeState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// `true` when `version` has not yet been recorded as shown.
pub fn should_notify_for_version(home: &Path, version: &str) -> bool {
    match load_update_notice(home).notified_version.as_deref() {
        Some(seen) if seen == version => false,
        _ => true,
    }
}

/// Persist that we have shown a notice for `version`.
pub fn record_notified_version(home: &Path, version: &str) -> std::io::Result<()> {
    let path = update_notice_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let state = UpdateNoticeState {
        notified_version: Some(version.to_string()),
    };
    let bytes = serde_json::to_vec_pretty(&state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, bytes)
}

/// Whether this version should be suppressed: already persisted, or dismissed in config.
pub fn notice_already_shown(home: &Path, version: &str, dismissed_version: Option<&str>) -> bool {
    dismissed_version == Some(version) || !should_notify_for_version(home, version)
}

/// If `version` has not been shown yet, persist it and return `true` (show now).
/// A write failure still returns `true` so this launch can notify; the next
/// launch may repeat if the once-flag never landed.
pub fn claim_update_notice(home: &Path, version: &str) -> bool {
    if !should_notify_for_version(home, version) {
        return false;
    }
    if let Err(e) = record_notified_version(home, version) {
        tracing::warn!(
            path = %update_notice_path(home).display(),
            error = %e,
            "failed to persist update notice once-flag"
        );
    }
    true
}

/// `$EZER_UPSTREAM_REPO` or [`DEFAULT_EZER_UPSTREAM_REPO`].
pub fn ezer_upstream_repo() -> String {
    std::env::var("EZER_UPSTREAM_REPO")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_EZER_UPSTREAM_REPO.to_string())
}

/// `owner/repo` that is not an xAI / ezer.com / X channel.
pub fn is_allowed_upstream_repo(repo: &str) -> bool {
    let repo = repo.trim();
    if repo.is_empty() || !repo.contains('/') || repo.contains("://") {
        return false;
    }
    let lower = repo.to_ascii_lowercase();
    if lower.contains("x.ai")
        || lower.contains("xai")
        || lower.contains("ezer")
        || lower.contains("twitter")
        || lower.split('/').next() == Some("x")
    {
        return false;
    }
    true
}

/// Strip a leading `v` and accept only semver tags.
pub fn version_from_release_tag(tag: &str) -> Option<String> {
    let t = tag.trim().trim_start_matches('v');
    semver::Version::parse(t).ok()?;
    Some(t.to_string())
}

/// Latest release tag from the allowed ezer upstream. `None` on network/404/xAI-bound repo.
pub async fn fetch_ezer_upstream_version() -> Option<String> {
    let repo = ezer_upstream_repo();
    if !is_allowed_upstream_repo(&repo) {
        tracing::info!(
            repo = %repo,
            "update notice skipped: upstream repo is xAI/X-bound"
        );
        return None;
    }
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client =
        ezer_extra_ca::build_reqwest_client(|builder| builder.timeout(Duration::from_secs(8)))
            .ok()?;
    let resp = client
        .get(&url)
        .header("User-Agent", "ezer")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    version_from_release_tag(body.get("tag_name")?.as_str()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_should_notify() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(should_notify_for_version(tmp.path(), "1.2.3"));
        assert!(!notice_already_shown(tmp.path(), "1.2.3", None));
    }

    #[test]
    fn claim_is_once_per_version() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(claim_update_notice(tmp.path(), "9.9.9"));
        assert!(!should_notify_for_version(tmp.path(), "9.9.9"));
        assert!(!claim_update_notice(tmp.path(), "9.9.9"));
        let raw = std::fs::read_to_string(update_notice_path(tmp.path())).unwrap();
        assert!(raw.contains("9.9.9"), "{raw}");
        assert_eq!(
            load_update_notice(tmp.path()).notified_version.as_deref(),
            Some("9.9.9")
        );
    }

    #[test]
    fn new_version_notifies_again() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(claim_update_notice(tmp.path(), "1.0.0"));
        assert!(should_notify_for_version(tmp.path(), "1.0.1"));
        assert!(claim_update_notice(tmp.path(), "1.0.1"));
        assert!(!should_notify_for_version(tmp.path(), "1.0.1"));
        assert_eq!(
            load_update_notice(tmp.path()).notified_version.as_deref(),
            Some("1.0.1")
        );
    }

    #[test]
    fn dismissed_version_suppresses_notice() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(notice_already_shown(tmp.path(), "2.0.0", Some("2.0.0")));
        assert!(!notice_already_shown(tmp.path(), "2.0.0", Some("1.0.0")));
    }

    #[test]
    fn corrupt_file_treated_as_never_notified() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(update_notice_path(tmp.path()), "not-json").unwrap();
        assert!(should_notify_for_version(tmp.path(), "0.1.0"));
    }

    #[test]
    fn default_ezer_repo_is_allowed() {
        assert!(is_allowed_upstream_repo(DEFAULT_EZER_UPSTREAM_REPO));
        assert!(is_allowed_upstream_repo("acme/tools"));
    }

    #[test]
    fn xai_bound_repos_are_rejected() {
        assert!(!is_allowed_upstream_repo("xai-org-shared/ezer-build"));
        assert!(!is_allowed_upstream_repo("xai-org/ezer"));
        assert!(!is_allowed_upstream_repo("foo/ezer-cli"));
        assert!(!is_allowed_upstream_repo("https://x.ai/cli"));
        assert!(!is_allowed_upstream_repo(""));
        assert!(!is_allowed_upstream_repo("nopath"));
    }

    #[test]
    fn release_tag_strips_v_and_requires_semver() {
        assert_eq!(version_from_release_tag("v1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(
            version_from_release_tag("0.1.220").as_deref(),
            Some("0.1.220")
        );
        assert_eq!(version_from_release_tag("latest"), None);
        assert_eq!(version_from_release_tag(""), None);
    }
}
