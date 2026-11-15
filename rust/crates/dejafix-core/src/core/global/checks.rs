use super::query::{GLOBAL_FIELDS, library_docs};
use super::types::{CheckFailure, CheckReport, CheckWarning, GotchaInput, Libraries, StoredGotcha};
use crate::core::rank::Index;
use crate::core::redact::{redact, redaction_notice};
use crate::core::text::{LanguageVerdict, contains_word, language_verdict};

const MIN_KEYWORDS: usize = 2;
const MAX_KEYWORDS: usize = 8;
const MAX_KEYWORD_WORDS: usize = 4;
const MIN_CHARS: usize = 30;
const MAX_CHARS: usize = 600;
const DUPLICATE_NUMERATOR: usize = 4;
const DUPLICATE_DENOMINATOR: usize = 5;

const ABSOLUTE_PATHS: [&str; 3] = ["/Users/", "/home/", "~/"];
const HOME_VARIABLES: [&str; 3] = ["$HOME/", "${HOME}/", "%USERPROFILE%"];
const SYSTEM_DIRECTORIES: [&str; 22] = [
    "Applications",
    "Library",
    "System",
    "bin",
    "boot",
    "dev",
    "etc",
    "home",
    "lib",
    "lib32",
    "lib64",
    "nix",
    "opt",
    "private",
    "proc",
    "run",
    "sbin",
    "snap",
    "sys",
    "tmp",
    "usr",
    "var",
];
const MONOREPO_PATHS: [&str; 4] = ["apps/", "libs/", "packages/", "tools/"];
const TICKET_EXCEPTIONS: [&str; 40] = [
    "AES", "AMD", "ANSI", "ARM", "AVX", "BIP", "BS", "CP", "CVE", "CWE", "ECMA", "EIP", "EN",
    "ERC", "ES", "FIPS", "GB", "HS", "IE", "IEC", "IEEE", "ISO", "ITU", "JEP", "JSR", "KEP", "MD",
    "NIST", "OWASP", "PEP", "POSIX", "PS", "RFC", "RS", "RSA", "SHA", "SSE", "TLS", "UTF", "WCAG",
];
const INTERNAL_SUFFIXES: [&str; 6] = [".int", ".local", ".internal", ".lan", ".corp", ".home"];
const INTERNAL_LABELS: [&str; 7] = [
    "corp", "int", "internal", "intra", "intranet", "lan", "local",
];
const REVERSE_DOMAIN_ROOTS: [&str; 9] = [
    "android", "com", "edu", "gov", "io", "java", "javax", "net", "org",
];
const GENERIC_DOMAINS: [&str; 8] = ["app", "biz", "cloud", "com", "dev", "info", "net", "org"];
const SHORT_EXTENSIONS: [&str; 18] = [
    "cs", "db", "el", "ex", "go", "gz", "hs", "js", "kt", "md", "ml", "pl", "py", "rb", "rs", "sh",
    "so", "ts",
];
const RUN_STATUS_PHRASES: [&str; 8] = [
    "not pushed",
    "not merged",
    "awaiting",
    "reverted",
    "retry of",
    "still failing",
    "work in progress",
    "wip",
];
const NUMBERED_STEPS: [&str; 3] = ["task", "round", "phase"];

fn is_path_char(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '/' | '.' | '_' | '-' | '~' | '@')
}

fn token_at(text: &str, at: usize) -> String {
    let tail = text.get(at..).unwrap_or_default();
    let end = tail
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '`' | '"' | '\'' | ',' | ')' | ';')
        })
        .unwrap_or(tail.len());
    tail.get(..end)
        .unwrap_or_default()
        .trim_end_matches(['.', ':'])
        .to_owned()
}

fn starts_word(text: &str, at: usize) -> bool {
    text.get(..at)
        .and_then(|head| head.chars().next_back())
        .is_none_or(|before| !is_path_char(before))
}

fn unc_share(text: &str, from: usize) -> bool {
    let tail = text.get(from..).unwrap_or_default();
    let host = tail
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
        .count();
    let share = tail.get(host..).unwrap_or_default().as_bytes();
    host >= 2 && matches!(share, [b'\\', first, ..] if first.is_ascii_alphanumeric())
}

fn project_path(text: &str) -> Option<String> {
    if let Some(at) = ABSOLUTE_PATHS
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
    {
        return Some(token_at(text, at));
    }
    let bytes = text.as_bytes();
    for (at, window) in bytes.windows(3).enumerate() {
        let windows_root =
            matches!(window, [drive, b':', b'\\' | b'/'] if drive.is_ascii_alphabetic());
        let share = window.starts_with(b"\\\\") && unc_share(text, at + 2);
        if (windows_root || share) && starts_word(text, at) {
            return Some(token_at(text, at));
        }
    }
    if let Some(at) = HOME_VARIABLES
        .iter()
        .flat_map(|marker| text.match_indices(marker).map(|(at, _)| at))
        .filter(|at| starts_word(text, *at))
        .min()
    {
        return Some(token_at(text, at));
    }
    text.char_indices()
        .map(|(at, _)| at)
        .filter(|at| starts_word(text, *at))
        .find(|at| {
            MONOREPO_PATHS
                .iter()
                .any(|prefix| text.get(*at..).is_some_and(|tail| tail.starts_with(prefix)))
        })
        .map(|at| token_at(text, at))
}

fn possible_path(text: &str) -> Option<String> {
    text.match_indices('/')
        .filter(|(at, _)| starts_word(text, *at))
        .map(|(at, _)| token_at(text, at))
        .find(|token| {
            if token.starts_with("//") {
                return false;
            }
            let segments: Vec<&str> = token
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect();
            segments.len() >= 3
                && segments
                    .first()
                    .is_some_and(|top| !SYSTEM_DIRECTORIES.contains(top))
        })
}

fn hash_ticket(text: &str) -> Option<String> {
    text.match_indices('#').find_map(|(at, _)| {
        let digits = text
            .get(at + 1..)?
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        (digits >= 3).then(|| text.get(at..at + 1 + digits).unwrap_or_default().to_owned())
    })
}

fn key_tickets(text: &str) -> impl Iterator<Item = (String, bool)> + '_ {
    let bytes = text.as_bytes();
    let mut at = 0;
    std::iter::from_fn(move || {
        while at < bytes.len() {
            let start = at;
            let previous = start.checked_sub(1).and_then(|before| bytes.get(before));
            let letters = bytes
                .get(start..)
                .unwrap_or_default()
                .iter()
                .take_while(|byte| byte.is_ascii_uppercase())
                .count();
            if letters == 0 || previous.is_some_and(u8::is_ascii_alphanumeric) {
                at += letters.max(1);
                continue;
            }
            let dash = start + letters;
            at = dash.max(start + 1);
            let digits = bytes
                .get(dash + 1..)
                .unwrap_or_default()
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            let end = dash + 1 + digits;
            let key = text.get(start..dash).unwrap_or_default();
            if letters >= 2
                && bytes.get(dash) == Some(&b'-')
                && digits >= 2
                && !bytes.get(end).is_some_and(u8::is_ascii_alphanumeric)
                && !TICKET_EXCEPTIONS.contains(&key)
            {
                let hard = letters >= 3 || digits >= 3;
                return text.get(start..end).map(|ticket| (ticket.to_owned(), hard));
            }
        }
        None
    })
}

fn key_ticket(text: &str) -> Option<String> {
    key_tickets(text)
        .find(|(_, hard)| *hard)
        .map(|(ticket, _)| ticket)
}

fn possible_ticket(text: &str) -> Option<String> {
    key_tickets(text)
        .find(|(_, hard)| !*hard)
        .map(|(ticket, _)| ticket)
}

fn is_private_ipv4(token: &str) -> bool {
    let mut octets = [0_u16; 4];
    let mut count = 0;
    for part in token.split('.') {
        let Some(slot) = octets.get_mut(count) else {
            return false;
        };
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        let Ok(value) = part.parse::<u16>() else {
            return false;
        };
        if value > 255 {
            return false;
        }
        *slot = value;
        count += 1;
    }
    count == 4
        && match octets {
            [10, ..] | [192, 168, ..] => true,
            [172, second, ..] => (16..=31).contains(&second),
            _ => false,
        }
}

fn ends_with_ignoring_case(text: &str, suffix: &str) -> bool {
    text.len() > suffix.len()
        && text
            .get(text.len() - suffix.len()..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
}

fn host_tokens(text: &str) -> impl Iterator<Item = (&str, bool)> {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    })
    .map(|token| (token.trim_matches(['.', '-']), token.starts_with('.')))
}

fn has_internal_label(token: &str) -> bool {
    let labels: Vec<&str> = token.split('.').collect();
    let (Some(first), Some(last)) = (labels.first(), labels.last()) else {
        return false;
    };
    labels.len() >= 3
        && !REVERSE_DOMAIN_ROOTS.contains(first)
        && last.bytes().all(|byte| byte.is_ascii_lowercase())
        && ((last.len() == 2 && !SHORT_EXTENSIONS.contains(last)) || GENERIC_DOMAINS.contains(last))
        && labels
            .get(1..labels.len() - 1)
            .unwrap_or_default()
            .iter()
            .any(|label| INTERNAL_LABELS.contains(&label.to_ascii_lowercase().as_str()))
}

fn internal_host(text: &str) -> Option<String> {
    host_tokens(text)
        .find(|(token, dotfile)| {
            is_private_ipv4(token)
                || (!dotfile
                    && (INTERNAL_SUFFIXES
                        .iter()
                        .any(|suffix| ends_with_ignoring_case(token, suffix))
                        || has_internal_label(token)))
        })
        .map(|(token, _)| token.to_owned())
}

fn bare_url_host(text: &str) -> Option<String> {
    text.match_indices("://").find_map(|(at, _)| {
        let rest = text.get(at + 3..)?;
        let authority_end = rest
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '?' | '#')
            })
            .unwrap_or(rest.len());
        let authority = rest.get(..authority_end)?;
        let rest = authority
            .rfind('@')
            .and_then(|at| authority.get(at + 1..))
            .unwrap_or(authority);
        let end = rest
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
            .unwrap_or(rest.len());
        let host = rest.get(..end)?;
        let ends_host = rest
            .get(end..)
            .is_none_or(|tail| !tail.starts_with(['.', '[', '$', '{', '<', '%', '_']));
        (!host.is_empty() && ends_host && !host.eq_ignore_ascii_case("localhost"))
            .then(|| host.to_owned())
    })
}

fn numbered_step(lowered: &str) -> Option<String> {
    NUMBERED_STEPS.iter().find_map(|step| {
        lowered.match_indices(step).find_map(|(at, _)| {
            if !starts_word(lowered, at) {
                return None;
            }
            let rest = lowered.get(at + step.len()..)?;
            let digits_from = rest.strip_prefix(' ')?;
            let digits = digits_from.bytes().take_while(u8::is_ascii_digit).count();
            (digits > 0).then(|| {
                lowered
                    .get(at..at + step.len() + 1 + digits)
                    .map(str::to_owned)
            })?
        })
    })
}

fn run_status(lowered: &str) -> Option<String> {
    RUN_STATUS_PHRASES
        .iter()
        .find(|phrase| contains_word(lowered, phrase))
        .map(|phrase| (*phrase).to_owned())
        .or_else(|| numbered_step(lowered))
}

#[must_use]
pub fn is_package_name(library: &str) -> bool {
    !library.is_empty()
        && library.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'@' | b'/' | b'.' | b'_' | b'-')
        })
}

#[must_use]
pub fn normalize_input(input: GotchaInput, libraries: &Libraries) -> GotchaInput {
    GotchaInput {
        library: libraries.canonical(&input.library),
        keywords: input
            .keywords
            .iter()
            .map(|keyword| keyword.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect(),
        gotcha: input.gotcha.trim().to_owned(),
        versions: input
            .versions
            .map(|versions| versions.trim().to_owned())
            .filter(|versions| !versions.is_empty()),
    }
}

fn language(input: &GotchaInput, report: &mut CheckReport) {
    let keywords = input.keywords.join(" ");
    for text in [input.gotcha.as_str(), keywords.as_str()] {
        if let LanguageVerdict::Rejected { reason } = language_verdict(text) {
            report
                .failures
                .push(CheckFailure::NotEnglish(reason.to_string()));
            return;
        }
    }
    if language_verdict(&input.gotcha) == LanguageVerdict::Doubtful {
        report.warnings.push(CheckWarning::Doubtful);
    }
}

fn shape(input: &GotchaInput, report: &mut CheckReport) {
    if !is_package_name(&input.library) {
        report.failures.push(CheckFailure::Library);
    }
    let count = input.keywords.len();
    if !(MIN_KEYWORDS..=MAX_KEYWORDS).contains(&count) {
        report.failures.push(CheckFailure::KeywordCount(count));
    }
    if input.keywords.iter().any(String::is_empty) {
        report.failures.push(CheckFailure::EmptyKeyword);
    }
    if let Some(long) = input
        .keywords
        .iter()
        .find(|keyword| keyword.split_whitespace().count() > MAX_KEYWORD_WORDS)
    {
        report
            .failures
            .push(CheckFailure::LongKeyword(long.clone()));
    }
    let length = input.gotcha.chars().count();
    if !(MIN_CHARS..=MAX_CHARS).contains(&length) {
        report.failures.push(CheckFailure::Length(length));
    }
}

fn details(input: &GotchaInput, projects: &[&str], report: &mut CheckReport) {
    let texts: Vec<&str> = std::iter::once(input.gotcha.as_str())
        .chain(input.keywords.iter().map(String::as_str))
        .collect();
    let first = |scan: fn(&str) -> Option<String>| texts.iter().find_map(|text| scan(text));
    if let Some(path) = first(project_path) {
        report.failures.push(CheckFailure::ProjectPath(path));
    }
    if let Some(ticket) = first(hash_ticket).or_else(|| first(key_ticket)) {
        report.failures.push(CheckFailure::Ticket(ticket));
    }
    if let Some(host) = first(internal_host) {
        report.failures.push(CheckFailure::InternalHost(host));
    }
    let path_refused = report
        .failures
        .iter()
        .any(|failure| matches!(failure, CheckFailure::ProjectPath(_)));
    if let Some(path) = first(possible_path).filter(|_| !path_refused) {
        report.warnings.push(CheckWarning::PossiblePath(path));
    }
    if let Some(ticket) = first(possible_ticket) {
        report.warnings.push(CheckWarning::PossibleTicket(ticket));
    }
    if let Some(host) = first(bare_url_host) {
        report.warnings.push(CheckWarning::PossibleHost(host));
    }
    let lowered = texts.join("\n").to_lowercase();
    if let Some(name) = projects
        .iter()
        .map(|name| name.to_lowercase())
        .find(|name| name.len() > 1 && *name != input.library && contains_word(&lowered, name))
    {
        report.warnings.push(CheckWarning::ProjectName(name));
    }
    if let Some(status) = run_status(&lowered) {
        report.warnings.push(CheckWarning::RunStatus(status));
    }
}

fn duplicate(
    input: &GotchaInput,
    libraries: &Libraries,
    existing: &[StoredGotcha],
    replacing: Option<&str>,
) -> Option<String> {
    let same: Vec<&StoredGotcha> = existing
        .iter()
        .filter(|record| {
            Some(record.id()) != replacing && libraries.canonical(record.library()) == input.library
        })
        .collect();
    if same.is_empty() {
        return None;
    }
    let docs = library_docs(same.iter().copied(), &input.library, libraries);
    let index = Index::build(GLOBAL_FIELDS, &docs);
    let text = format!("{} {}", input.gotcha, input.keywords.join(" "));
    let terms = index.query_terms(&text);
    if terms.distinct == 0 {
        return None;
    }
    (0..u32::try_from(same.len()).unwrap_or(u32::MAX))
        .map(|doc| (doc, index.terms_covered(doc, &terms.ids)))
        .filter(|(_, covered)| {
            covered * DUPLICATE_DENOMINATOR >= terms.distinct * DUPLICATE_NUMERATOR
        })
        .max_by_key(|(doc, covered)| (*covered, std::cmp::Reverse(*doc)))
        .and_then(|(doc, _)| same.get(usize::try_from(doc).ok()?))
        .map(|record| record.id().to_owned())
}

#[must_use]
pub fn check_gotcha(
    input: &GotchaInput,
    libraries: &Libraries,
    projects: &[&str],
    existing: &[StoredGotcha],
    replacing: Option<&str>,
) -> CheckReport {
    let mut report = CheckReport::default();
    language(input, &mut report);
    shape(input, &mut report);
    details(input, projects, &mut report);
    if let Some(id) = duplicate(input, libraries, existing, replacing) {
        report.failures.push(CheckFailure::Duplicate(id.clone()));
        report.duplicate_of = Some(id);
    }
    report
}

fn redacted(text: &str, fired: &mut Vec<crate::core::redact::SecretKind>) -> String {
    let redaction = redact(text);
    fired.extend(redaction.fired);
    redaction.text.into_owned()
}

#[must_use]
pub fn redact_input(input: &GotchaInput) -> (GotchaInput, Option<String>) {
    let mut fired = Vec::new();
    let clean = GotchaInput {
        library: input.library.clone(),
        keywords: input
            .keywords
            .iter()
            .map(|keyword| redacted(keyword, &mut fired))
            .collect(),
        gotcha: redacted(&input.gotcha, &mut fired),
        versions: input
            .versions
            .as_deref()
            .map(|versions| redacted(versions, &mut fired)),
    };
    (clean, redaction_notice(&fired))
}
