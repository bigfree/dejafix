use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig};

const MAX_BODY_BYTES: u64 = 1024 * 1024;

pub fn latest_version(url: &str, user_agent: &str, timeout: Duration) -> Option<String> {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let agent: Agent = Agent::config_builder()
        .tls_config(tls)
        .user_agent(user_agent)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut response = agent.get(url).call().ok()?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY_BYTES)
        .read_to_vec()
        .ok()?;
    let document: Value = serde_json::from_slice(&bytes).ok()?;
    document
        .get("info")?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}
