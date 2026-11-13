use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha1::{Digest, Sha1};

use super::types::{Ids, OsIds};
use crate::core::event::EventId;

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Ids for OsIds {
    fn event_id(&mut self) -> EventId<'static> {
        EventId::from_string(format!("evt_{}", uuid4_hex(20)))
    }

    fn entry_id(&mut self, prefix: &'static str) -> String {
        format!("{prefix}_{}", uuid4_hex(12))
    }
}

pub(crate) fn uuid4_hex(length: usize) -> String {
    let mut bytes = [0_u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        bytes = fallback_bytes();
    }
    if let Some(byte) = bytes.get_mut(6) {
        *byte = (*byte & 0x0f) | 0x40;
    }
    if let Some(byte) = bytes.get_mut(8) {
        *byte = (*byte & 0x3f) | 0x80;
    }
    let mut hex = String::with_capacity(32);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex.get(..length).unwrap_or(&hex).to_owned()
}

fn fallback_bytes() -> [u8; 16] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut hasher = Sha1::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(digest.iter()) {
        *slot = *byte;
    }
    bytes
}
