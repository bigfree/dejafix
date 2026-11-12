use std::borrow::Cow;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::types::{Redaction, SecretKind};
use crate::core::text::PYTHON_SPACE_CLASS;

pub const REDACTABLE_FIELDS: [&str; 5] = ["summary", "notes", "command", "git_message", "location"];

const SOURCES: [(SecretKind, &str); 10] = [
    (SecretKind::OpenaiKey, r"\bsk-[A-Za-z0-9_-]{40,}\b"),
    (SecretKind::GithubPat, r"\bgithub_pat_[A-Za-z0-9_]{82}\b"),
    (SecretKind::GithubToken, r"\bgh[pousr]_[A-Za-z0-9]{36}\b"),
    (SecretKind::AwsAccessKey, r"\bAKIA[0-9A-Z]{16}\b"),
    (SecretKind::GoogleApiKey, r"\bAIza[0-9A-Za-z_-]{35}\b"),
    (SecretKind::SlackToken, r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"),
    (
        SecretKind::StripeKey,
        r"\b(?:sk|pk|rk)_(?:live|test)_[A-Za-z0-9]{24,}\b",
    ),
    (
        SecretKind::Jwt,
        r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{10,}\b",
    ),
    (
        SecretKind::BearerToken,
        r"[Bb]earer{SPACE}+[A-Za-z0-9._~+/=\-]{20,}",
    ),
    (
        SecretKind::PrivateKey,
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
    ),
];

static PATTERNS: LazyLock<Vec<(SecretKind, Regex)>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .filter_map(|(kind, source)| {
            Regex::new(&source.replace("{SPACE}", PYTHON_SPACE_CLASS))
                .ok()
                .map(|regex| (*kind, regex))
        })
        .collect()
});

impl SecretKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiKey => "openai_key",
            Self::GithubPat => "github_pat",
            Self::GithubToken => "github_token",
            Self::AwsAccessKey => "aws_access_key",
            Self::GoogleApiKey => "google_api_key",
            Self::SlackToken => "slack_token",
            Self::StripeKey => "stripe_key",
            Self::Jwt => "jwt",
            Self::BearerToken => "bearer_token",
            Self::PrivateKey => "private_key",
        }
    }
}

#[must_use]
pub fn redact(text: &str) -> Redaction<'_> {
    let mut fired = Vec::new();
    let mut current = Cow::Borrowed(text);
    if text.is_empty() {
        return Redaction {
            text: current,
            fired,
        };
    }
    for (kind, regex) in PATTERNS.iter() {
        let replaced = if *kind == SecretKind::BearerToken {
            replace_after_non_letter(&current, regex, *kind, &mut fired)
        } else {
            replace_all(&current, regex, *kind, &mut fired)
        };
        if let Some(replaced) = replaced {
            current = Cow::Owned(replaced);
        }
    }
    Redaction {
        text: current,
        fired,
    }
}

fn marker(kind: SecretKind) -> String {
    format!("[REDACTED:{}]", kind.as_str())
}

fn replace_all(
    text: &str,
    regex: &Regex,
    kind: SecretKind,
    fired: &mut Vec<SecretKind>,
) -> Option<String> {
    let before = fired.len();
    let replaced = regex.replace_all(text, |_: &Captures| {
        fired.push(kind);
        marker(kind)
    });
    (fired.len() > before).then(|| replaced.into_owned())
}

fn replace_after_non_letter(
    text: &str,
    regex: &Regex,
    kind: SecretKind,
    fired: &mut Vec<SecretKind>,
) -> Option<String> {
    let mut out = String::new();
    let mut copied = 0;
    let mut at = 0;
    while let Some(found) = regex.find_at(text, at) {
        let blocked = text
            .get(..found.start())
            .and_then(|head| head.chars().next_back())
            .is_some_and(|character| character.is_ascii_alphabetic());
        if blocked {
            at = found.start()
                + text
                    .get(found.start()..)
                    .and_then(|tail| tail.chars().next())
                    .map_or(1, char::len_utf8);
            continue;
        }
        out.push_str(text.get(copied..found.start())?);
        out.push_str(&marker(kind));
        fired.push(kind);
        copied = found.end();
        at = found.end();
    }
    if copied == 0 {
        return None;
    }
    out.push_str(text.get(copied..)?);
    Some(out)
}

#[must_use]
pub fn redaction_notice(fired: &[SecretKind]) -> Option<String> {
    if fired.is_empty() {
        return None;
    }
    let mut kinds: Vec<&str> = fired.iter().map(|kind| kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    Some(format!(
        "dejafix: redacted {} secret(s) before write ({})",
        fired.len(),
        kinds.join(", ")
    ))
}

#[cfg(test)]
pub(super) fn compiled() -> usize {
    PATTERNS.len()
}
