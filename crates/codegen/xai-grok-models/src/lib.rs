//! Default model IDs loaded from `default_models.json` at runtime.
//! Edit that JSON file to change them.
//!
//! At runtime each model is resolved from the first of these that is set: CLI flag, ENV var, config.toml, remote settings, these defaults.
//!
//! Catalog keys vs wire slugs: `default` / `web_search` / `image_description` /
//! `session_summary` name **catalog entries** (`id`, falling back to `model`).
//! `model` is the API slug (e.g. `id = "workbuddy"`, `model = "deepseek-v4.1-flash"`).
//! Startup used to validate only `model` and panic after the strip-grok merge
//! when `default` was `workbuddy`.

#![deny(clippy::indexing_slicing)]

use std::sync::LazyLock;

/// The raw JSON, embedded at compile time.
/// It is `pub` because `xai_grok_shell::models` re-exports it and `agent::config` reads it.
pub const DEFAULT_MODELS_JSON: &str = include_str!("../default_models.json");

#[derive(serde::Deserialize)]
struct DefaultModels {
    default: String,
    /// Falls back to `default` if not specified in JSON.
    web_search: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    image_description: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    session_summary: Option<String>,
    models: Vec<DefaultModelEntry>,
}

#[derive(serde::Deserialize)]
struct DefaultModelEntry {
    /// Catalog key used by `default` / config `[models]` / `--model`.
    /// Matches `agent::config::default_models`, which keys the map on `id`.
    #[serde(default)]
    id: Option<String>,
    /// Wire slug sent in API requests (may differ from `id`).
    model: String,
}

impl DefaultModelEntry {
    /// Same key `agent::config::default_models` uses for the catalog map.
    fn catalog_key(&self) -> &str {
        self.id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or(self.model.as_str())
    }
}

fn assert_catalog_contains(catalog: &[&str], field: &str, value: &str) {
    assert!(
        catalog.contains(&value),
        "default_models.json: '{field}' is '{value}' but catalog keys are {catalog:?}"
    );
}

static DEFAULTS: LazyLock<DefaultModels> = LazyLock::new(|| {
    let defaults: DefaultModels = serde_json::from_str(DEFAULT_MODELS_JSON)
        .expect("default_models.json: invalid JSON or missing 'default' field");

    // Baked-in JSON: a mismatch here is a developer error. Compare against
    // catalog keys (`id` else `model`), not wire slugs alone — otherwise an
    // alias like workbuddy → deepseek-v4.1-flash panics at process start.
    let catalog: Vec<&str> = defaults
        .models
        .iter()
        .map(DefaultModelEntry::catalog_key)
        .collect();
    assert_catalog_contains(&catalog, "default", &defaults.default);
    if let Some(ref value) = defaults.web_search {
        assert_catalog_contains(&catalog, "web_search", value);
    }
    if let Some(ref value) = defaults.image_description {
        assert_catalog_contains(&catalog, "image_description", value);
    }
    if let Some(ref value) = defaults.session_summary {
        assert_catalog_contains(&catalog, "session_summary", value);
    }

    defaults
});

/// Primary model for coding tasks and general fallback.
pub fn default_model() -> &'static str {
    &DEFAULTS.default
}

/// Model for web search tool synthesis. Falls back to default model.
pub fn default_web_search_model() -> &'static str {
    DEFAULTS.web_search.as_deref().unwrap_or(&DEFAULTS.default)
}

/// Model for image describe. Falls back to default model.
pub fn default_image_description_model() -> &'static str {
    DEFAULTS
        .image_description
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

/// Model for session title generation. Falls back to default model.
pub fn default_session_summary_model() -> &'static str {
    DEFAULTS
        .session_summary
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baked_defaults_resolve_without_panic() {
        assert_eq!(default_model(), "workbuddy");
        assert_eq!(default_web_search_model(), "workbuddy");
        assert_eq!(default_image_description_model(), "workbuddy");
        assert_eq!(default_session_summary_model(), "workbuddy");
    }

    #[test]
    fn catalog_key_uses_id_when_wire_slug_differs() {
        let entry: DefaultModelEntry =
            serde_json::from_str(r#"{"id":"workbuddy","model":"deepseek-v4.1-flash"}"#)
                .expect("entry");
        assert_eq!(entry.catalog_key(), "workbuddy");
        assert_eq!(entry.model, "deepseek-v4.1-flash");
    }

    #[test]
    fn catalog_key_falls_back_to_model() {
        let entry: DefaultModelEntry = serde_json::from_str(r#"{"model":"hy3"}"#).expect("entry");
        assert_eq!(entry.catalog_key(), "hy3");
    }

    #[test]
    fn baked_json_default_is_a_catalog_key() {
        let root: serde_json::Value = serde_json::from_str(DEFAULT_MODELS_JSON).expect("json");
        let default = root["default"].as_str().expect("default");
        let keys: Vec<String> = root["models"]
            .as_array()
            .expect("models")
            .iter()
            .map(|m| {
                m.get("id")
                    .and_then(|v| v.as_str())
                    .filter(|id| !id.is_empty())
                    .or_else(|| m.get("model").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_owned()
            })
            .collect();
        assert!(
            keys.iter().any(|k| k == default),
            "default {default:?} missing from catalog keys {keys:?}"
        );
    }
}
