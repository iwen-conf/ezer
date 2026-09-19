pub const FIRST_PARTY_CREDENTIAL_ENV_VARS: &[&str] = &[
    "EZER_AUTH",
    "EZER_AUTH_PATH",
    "EZER_API_KEY",
    "XAI_API_KEY",
    "EZER_DEPLOYMENT_KEY",
    "EZER_CODE_XAI_API_KEY",
    "EZER_EXTRA_AUTH_KEY",
    "EZER_TRACE_UPLOAD_CREDENTIALS_FILE",
    "OTEL_EXPORTER_OTLP_HEADERS",
    "EZER_INTERNAL_OTLP_HEADERS",
];

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enabled" => Some(true),
        "0" | "false" | "no" | "off" | "disabled" => Some(false),
        _ => None,
    }
}

pub fn env_bool(name: &str) -> Option<bool> {
    parse_bool(&std::env::var(name).ok()?)
}

pub fn env_string(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Opt-in xAI / grok.com browser login and xAI-sourced UI (announcements, remote banners, login nudges).
/// Off by default so BYOK ezer users never see notices fetched from xAI or X.
pub fn xai_login_enabled() -> bool {
    env_bool("EZER_ENABLE_XAI_LOGIN") == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EnvVarGuard;

    #[test]
    fn parse_bool_reads_known_spellings() {
        for on in ["1", "true", "YES", "On", "enabled"] {
            assert_eq!(parse_bool(on), Some(true), "{on}");
        }
        for off in ["0", "false", "NO", "Off", "disabled"] {
            assert_eq!(parse_bool(off), Some(false), "{off}");
        }
        for none in ["", "  ", "maybe", "2"] {
            assert_eq!(parse_bool(none), None, "{none:?}");
        }
    }

    #[test]
    fn env_string_trims_and_treats_blank_as_unset() {
        let guard = EnvVarGuard::set("EZER_TEST_ENV_STRING", "  hi  ");
        assert_eq!(env_string("EZER_TEST_ENV_STRING"), Some("hi".to_string()));
        guard.set_value("   ");
        assert_eq!(env_string("EZER_TEST_ENV_STRING"), None);
    }

    #[test]
    fn xai_login_enabled_is_off_by_default() {
        let _guard = EnvVarGuard::remove("EZER_ENABLE_XAI_LOGIN");
        assert!(!xai_login_enabled());
    }

    #[test]
    fn xai_login_enabled_reads_truthy_flag() {
        let _guard = EnvVarGuard::set("EZER_ENABLE_XAI_LOGIN", "1");
        assert!(xai_login_enabled());
    }
}
