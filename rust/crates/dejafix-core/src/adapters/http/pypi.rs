use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig};

const MAX_BODY_BYTES: u64 = 1024 * 1024;
const MAX_VERSION_CHARS: usize = 64;
const GRACE: Duration = Duration::from_millis(500);

fn fetch(url: &str, user_agent: &str, timeout: Duration) -> Option<String> {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let agent: Agent = Agent::config_builder()
        .tls_config(tls)
        .user_agent(user_agent)
        .timeout_global(Some(timeout))
        .max_redirects(0)
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
        .filter(|version| {
            (1..=MAX_VERSION_CHARS).contains(&version.len())
                && version.bytes().all(|byte| byte.is_ascii_graphic())
        })
        .map(str::to_owned)
}

#[must_use]
pub fn latest_version(url: &str, user_agent: &str, timeout: Duration) -> Option<String> {
    let (sender, receiver) = mpsc::channel();
    let (url, user_agent) = (url.to_owned(), user_agent.to_owned());
    thread::Builder::new()
        .name("pypi".to_owned())
        .spawn(move || {
            let _ = sender.send(fetch(&url, &user_agent, timeout));
        })
        .ok()?;
    receiver.recv_timeout(timeout + GRACE).ok().flatten()
}
