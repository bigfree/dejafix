use std::borrow::Cow;

use super::patterns::redact;
use super::types::SecretKind;
use crate::core::event::{Event, Summary};

pub fn redact_event(event: &mut Event<'_>) -> Vec<SecretKind> {
    let mut fired = Vec::new();
    let summary = redact(event.summary.as_str());
    if !summary.fired.is_empty() {
        fired.extend(summary.fired);
        if let Some(redacted) = Summary::new(Cow::Owned(summary.text.into_owned())) {
            event.summary = redacted;
        }
    }
    for field in [
        &mut event.notes,
        &mut event.command,
        &mut event.origin.git_message,
        &mut event.location,
    ] {
        let Some(text) = field.as_deref() else {
            continue;
        };
        let redacted = redact(text);
        if redacted.fired.is_empty() {
            continue;
        }
        fired.extend(redacted.fired);
        *field = Some(Cow::Owned(redacted.text.into_owned()));
    }
    fired
}
