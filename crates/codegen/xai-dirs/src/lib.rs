//! Home-directory resolution generally: USERPROFILE-first `home_dir`, plus
//! ezer-home (`$EZER_HOME` or `<home>/.ezer`).
//! Shared by `ezer-config` and `xai-fast-worktree`.
//!
//! Which function to call:
//! - [`ezer_home`]: the usual choice, a cached, created path to build on.
//! - [`user_ezer_home`]: `None` instead of a cwd fallback when no home resolves.
//! - [`default_ezer_home`]: the `<home>/.ezer` default, ignoring env overrides, so callers can detect an override.
//! - [`resolve_ezer_home`]: a fresh, uncached resolve.
//! - [`resolve_ezer_home_with_source`]: [`resolve_ezer_home`] plus where the path came from.
//! - [`home_dir`]: the home directory itself, for sibling dot dirs (`~/.claude`, `~/.agents`, ...).
//!
//! TODO: collapse these getters by threading the path through config as an
//! explicit value.

#![deny(clippy::indexing_slicing)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Env override for the user config home (`~/.ezer`).
pub const EZER_HOME_ENV: &str = "EZER_HOME";
/// Default directory name under `$HOME`.
pub const DEFAULT_DOT_DIR: &str = ".ezer";

/// Where a resolved ezer home came from, so "why did ezer pick this
/// directory?" is answerable in diagnostics without re-reading the
/// environment at the asking site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EzerHomeSource {
    /// A non-empty `$EZER_HOME` override.
    EnvOverride,
    /// `<home>/.ezer` derived from the home directory.
    HomeDefault,
}

/// The user's home directory via [`std::env::home_dir`]: `HOME` on Unix, `USERPROFILE` on Windows.
/// Not `dirs::home_dir()`: on Windows `dirs` ignores a redirected `USERPROFILE`.
/// Every home-anchored path must come from this one function.
#[allow(deprecated, clippy::disallowed_methods)] // the one sanctioned std::env::home_dir call
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// `<home>/.ezer`, canonicalized via `dunce` (not `std::fs::canonicalize`,
/// which yields Windows `\\?\` verbatim paths).
fn ezer_home_in(home: &Path) -> PathBuf {
    dunce::canonicalize(home)
        .unwrap_or_else(|_| home.to_path_buf())
        .join(DEFAULT_DOT_DIR)
}

/// Non-empty `$EZER_HOME`.
fn home_env_override(ezer_home_env: Option<&OsStr>) -> Option<PathBuf> {
    ezer_home_env
        .filter(|env| !env.is_empty())
        .map(PathBuf::from)
}

/// `$EZER_HOME` verbatim when non-empty, else `<home>/.ezer`.
/// Used as-is (not canonicalized) so literal prefix checks and symlink guards still see original components.
fn resolve_ezer_home_from(
    ezer_home_env: Option<&OsStr>,
    os_home: Option<&Path>,
) -> Option<(PathBuf, EzerHomeSource)> {
    if let Some(env) = home_env_override(ezer_home_env) {
        return Some((env, EzerHomeSource::EnvOverride));
    }
    os_home.map(|home| (ezer_home_in(home), EzerHomeSource::HomeDefault))
}

/// Resolve the ezer home from the environment (fresh, no cache); `None` if neither resolves.
pub fn resolve_ezer_home() -> Option<PathBuf> {
    resolve_ezer_home_with_source().map(|(home, _)| home)
}

/// [`resolve_ezer_home`] plus the [`EzerHomeSource`] the path came from.
pub fn resolve_ezer_home_with_source() -> Option<(PathBuf, EzerHomeSource)> {
    resolve_ezer_home_from(
        std::env::var_os(EZER_HOME_ENV).as_deref(),
        home_dir().as_deref(),
    )
}

/// The default `<home>/.ezer`, used when `$EZER_HOME` is unset.
pub fn default_ezer_home() -> PathBuf {
    ezer_home_in(&home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

/// The ezer home, created if missing and cached for the process; falls back to
/// [`default_ezer_home`] when neither an env override nor a home resolves.
pub fn ezer_home() -> PathBuf {
    static EZER_HOME_CACHE: OnceLock<PathBuf> = OnceLock::new();
    EZER_HOME_CACHE
        .get_or_init(|| {
            let home = resolve_ezer_home().unwrap_or_else(default_ezer_home);
            if let Err(err) = std::fs::create_dir_all(&home) {
                tracing::warn!(path = %home.display(), %err, "failed to create ezer home");
            }
            home
        })
        .clone()
}

/// Like [`ezer_home`], but `None` when no home resolves (no cwd fallback).
pub fn user_ezer_home() -> Option<PathBuf> {
    resolve_ezer_home().is_some().then(ezer_home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::ffi::OsString;

    #[test]
    fn ezer_env_wins_over_os_home() {
        let resolved =
            resolve_ezer_home_from(Some(OsStr::new("/custom/ezer")), Some(Path::new("/home/u")));
        assert_eq!(
            resolved,
            Some((PathBuf::from("/custom/ezer"), EzerHomeSource::EnvOverride))
        );
    }

    #[test]
    fn unset_ezer_home_uses_dot_ezer_under_os_home() {
        let resolved = resolve_ezer_home_from(None, Some(Path::new("/home/u")));
        assert_eq!(
            resolved,
            Some((
                ezer_home_in(Path::new("/home/u")),
                EzerHomeSource::HomeDefault
            ))
        );
    }

    #[test]
    fn env_used_verbatim_even_when_it_exists() {
        // A real, existing dir whose canonical form differs (macOS symlinks
        // `/var` -> `/private/var`): the env value must come back unchanged.
        let tmp = tempfile::tempdir().unwrap();
        let resolved = resolve_ezer_home_from(Some(tmp.path().as_os_str()), None);
        assert_eq!(
            resolved,
            Some((tmp.path().to_path_buf(), EzerHomeSource::EnvOverride))
        );
    }

    #[test]
    fn empty_env_falls_through_to_os_home() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = resolve_ezer_home_from(Some(&OsString::new()), Some(tmp.path()));
        assert_eq!(
            resolved,
            Some((
                dunce::canonicalize(tmp.path()).unwrap().join(".ezer"),
                EzerHomeSource::HomeDefault
            ))
        );
    }

    #[test]
    fn default_home_is_ezer_dotdir() {
        let home = default_ezer_home();
        assert!(!home.to_string_lossy().starts_with(r"\\?\"));
        assert!(home.ends_with(".ezer"));
    }

    #[test]
    fn none_when_nothing_resolves() {
        assert_eq!(resolve_ezer_home_from(None, None), None);
    }
}
