//! HTTPS for suggestions and the gallery: Windows' own TLS and certificate
//! store (like a browser), so the build stays free of C code.

use std::time::Duration;

/// Largest response Flick accepts (full-size wallpapers can be big).
const MAX_BYTES: u64 = 40 * 1024 * 1024;

pub fn agent(timeout: Duration) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// The body of a GET request.
pub fn get_bytes(agent: &ureq::Agent, url: &str) -> anyhow::Result<Vec<u8>> {
    Ok(agent
        .get(url)
        .call()?
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()?)
}
