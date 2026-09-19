/// Apply auth headers to outbound visibility requests.
/// Implemented by `ezer-shell::util::ezer_auth_credentials::EzerAuthCredentials`.
/// Shell owns credential construction; data-collector builds the request without importing shell types.
pub trait HttpAuth: Send + Sync {
    fn apply(&self, builder: reqwest::RequestBuilder, base_url: &str) -> reqwest::RequestBuilder;
}
