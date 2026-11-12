mod patterns;
mod types;

pub use patterns::{REDACTABLE_FIELDS, redact, redaction_notice};
pub use types::{Redaction, SecretKind};

#[cfg(test)]
mod tests;
