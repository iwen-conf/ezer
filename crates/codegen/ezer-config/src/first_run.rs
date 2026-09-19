//! First-run BYOK `config.toml` for standalone ezer.
//!
//! Written only when the user home has no `config.toml` yet and this is not a
//! `$GROK_HOME`-only test override. The template is OpenAI Responses-first and
//! points at the WorkBuddy2API-Hub style gateway.

use std::fmt::Write as _;
use std::path::Path;

use crate::loader::USER_CONFIG_FILENAME;

/// Default OpenAI-compatible gateway used in first-run config and docs.
pub const DEFAULT_GATEWAY_BASE_URL: &str = "http://192.168.0.63:8788/v1";
/// Catalog key for the example BYOK model.
pub const DEFAULT_GATEWAY_MODEL_KEY: &str = "workbuddy";
/// Wire model id sent to the gateway (one of the ids this hub commonly exposes).
pub const DEFAULT_GATEWAY_MODEL_ID: &str = "deepseek-v4.1-flash";
/// Working key for the LAN WorkBuddy2API-Hub instance (user-supplied).
pub const DEFAULT_GATEWAY_API_KEY: &str = "SrdCiNW_1c1qkc--o6e_Btot7yCwa8JswK3N856Q6ck";
/// Extra wire ids commonly listed by that hub.
pub const DEFAULT_GATEWAY_OPTIONAL_MODELS: &[&str] = &[
    "deepseek-v4.1-flash-low",
    "deepseek-v4.1-flash-high",
    "deepseek-v4.1-flash-max",
    "hy4-preview-f",
    "hy3",
];

fn model_table(
    out: &mut String,
    catalog_key: &str,
    wire_id: &str,
    name: &str,
    base: &str,
    api_key: &str,
) {
    let _ = writeln!(
        out,
        r#"
[model."{key}"]
model = "{id}"
name = "{name}"
base_url = "{base}"
api_backend = "responses"
context_window = 200000
reasoning_effort = "max"
supports_reasoning_effort = true
api_key = "{api_key}"
env_key = ["EZER_API_KEY", "XAI_API_KEY"]
"#,
        key = catalog_key,
        id = wire_id,
        name = name,
        base = base,
        api_key = api_key,
    );
}

/// Starter `~/.ezer/config.toml` for a Responses BYOK gateway.
pub fn default_byok_config_toml() -> String {
    let mut toml = format!(
        r#"# ezer — BYOK-first default (OpenAI Responses).
# No xAI login is required.
#
# Gateway (WorkBuddy2API-Hub style):
#   POST {{base_url}}/responses          (default)
#   POST {{base_url}}/chat/completions   (secondary)
# Auth: Authorization: Bearer <key>, also x-api-key and api-key.

[auth]
preferred_method = "api_key"

[endpoints]
models_base_url = "{base}"

[models]
default = "{key}"
session_summary = "{key}"
"#,
        base = DEFAULT_GATEWAY_BASE_URL,
        key = DEFAULT_GATEWAY_MODEL_KEY,
    );
    model_table(
        &mut toml,
        DEFAULT_GATEWAY_MODEL_KEY,
        DEFAULT_GATEWAY_MODEL_ID,
        "WorkBuddy DeepSeek",
        DEFAULT_GATEWAY_BASE_URL,
        DEFAULT_GATEWAY_API_KEY,
    );
    for id in DEFAULT_GATEWAY_OPTIONAL_MODELS {
        model_table(
            &mut toml,
            id,
            id,
            id,
            DEFAULT_GATEWAY_BASE_URL,
            DEFAULT_GATEWAY_API_KEY,
        );
    }
    toml
}

/// Whether this process should seed a missing `config.toml`.
///
/// Tests and legacy `$GROK_HOME` overrides must not receive a surprise write.
/// A real first run (no env, or `$EZER_HOME`) does.
pub fn should_write_first_run_config() -> bool {
    if ezer_env::env_bool("EZER_SKIP_DEFAULT_CONFIG") == Some(true) {
        return false;
    }
    let ezer = std::env::var_os(xai_dirs::EZER_HOME_ENV)
        .filter(|v| !v.is_empty())
        .is_some();
    let legacy_home = std::env::var_os(xai_dirs::GROK_HOME_ENV)
        .filter(|v| !v.is_empty())
        .is_some();
    ezer || !legacy_home
}

/// Create `{home}/config.toml` from [`default_byok_config_toml`] when missing.
/// Returns `true` when a file was written.
pub fn ensure_first_run_config(home: &Path) -> bool {
    if !should_write_first_run_config() {
        return false;
    }
    let path = home.join(USER_CONFIG_FILENAME);
    if path.exists() {
        return false;
    }
    if let Err(err) = std::fs::create_dir_all(home) {
        tracing::warn!(path = %home.display(), %err, "failed to create ezer home for first-run config");
        return false;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            if let Err(err) = file.write_all(default_byok_config_toml().as_bytes()) {
                tracing::warn!(path = %path.display(), %err, "failed to write first-run ezer config");
                return false;
            }
            tracing::info!(path = %path.display(), "wrote first-run BYOK config.toml");
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "failed to create first-run ezer config");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn template_is_responses_byok() {
        let toml = default_byok_config_toml();
        assert!(toml.contains("api_backend = \"responses\""));
        assert!(toml.contains(DEFAULT_GATEWAY_BASE_URL));
        assert!(toml.contains("preferred_method = \"api_key\""));
        assert!(toml.contains(DEFAULT_GATEWAY_API_KEY));
        assert!(toml.contains("reasoning_effort = \"max\""));
        assert!(toml.contains("session_summary = \"workbuddy\""));
        assert!(toml.contains("model = \"deepseek-v4.1-flash\""));
        assert!(toml.contains("deepseek-v4.1-flash-low"));
        assert!(toml.contains("hy4-preview-f"));
        assert!(toml.contains("hy3"));
        assert!(!toml.contains("auth.x.ai"));
        assert!(!toml.contains("cli-chat-proxy"));
    }

    #[test]
    fn writes_once_when_missing() {
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var(xai_dirs::EZER_HOME_ENV, tmp.path());
            std::env::remove_var(xai_dirs::GROK_HOME_ENV);
            std::env::remove_var("EZER_SKIP_DEFAULT_CONFIG");
        }
        assert!(ensure_first_run_config(tmp.path()));
        assert!(!ensure_first_run_config(tmp.path()));
        let body = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
        assert!(body.contains("api_backend = \"responses\""));
        assert!(body.contains(DEFAULT_GATEWAY_API_KEY));
        unsafe {
            std::env::remove_var(xai_dirs::EZER_HOME_ENV);
        }
    }
}
