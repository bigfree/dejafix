use super::query::{GLOBAL_FIELDS, global_docs};
use super::types::{CheckFailure, CheckReport, CheckWarning, GotchaInput, Libraries, StoredGotcha};
use crate::core::rank::Index;
use crate::core::text::{LanguageVerdict, contains_word, language_verdict};

const MIN_KEYWORDS: usize = 2;
const MAX_KEYWORDS: usize = 8;
const MAX_KEYWORD_WORDS: usize = 4;
const MIN_CHARS: usize = 30;
const MAX_CHARS: usize = 600;
const DUPLICATE_NUMERATOR: usize = 4;
const DUPLICATE_DENOMINATOR: usize = 5;

const ABSOLUTE_PATHS: [&str; 3] = ["/Users/", "/home/", "~/"];
const MONOREPO_PATHS: [&str; 4] = ["apps/", "libs/", "packages/", "tools/"];
const TICKET_EXCEPTIONS: [&str; 8] = ["CVE", "ECMA", "IEEE", "ISO", "RFC", "SHA", "TLS", "UTF"];
const INTERNAL_SUFFIXES: [&str; 6] = [".int", ".local", ".internal", ".lan", ".corp", ".home"];
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
        if let [drive, b':', b'\\'] = window
            && drive.is_ascii_alphabetic()
            && starts_word(text, at)
        {
            return Some(token_at(text, at));
        }
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

fn key_ticket(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let previous = at.checked_sub(1).and_then(|before| bytes.get(before));
        let letters = bytes
            .get(at..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| byte.is_ascii_uppercase())
            .count();
        if letters == 0 || previous.is_some_and(u8::is_ascii_alphanumeric) {
            at += letters.max(1);
            continue;
        }
        let dash = at + letters;
        let digits = bytes
            .get(dash + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let end = dash + 1 + digits;
        let key = text.get(at..dash).unwrap_or_default();
        if letters >= 3
            && bytes.get(dash) == Some(&b'-')
            && digits >= 2
            && !bytes.get(end).is_some_and(u8::is_ascii_alphanumeric)
            && !TICKET_EXCEPTIONS.contains(&key)
        {
            return text.get(at..end).map(str::to_owned);
        }
        at = dash.max(at + 1);
    }
    None
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

fn internal_host(text: &str) -> Option<String> {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    })
    .map(|token| token.trim_matches(['.', '-']))
    .find(|token| {
        is_private_ipv4(token)
            || INTERNAL_SUFFIXES
                .iter()
                .any(|suffix| ends_with_ignoring_case(token, suffix))
    })
    .map(str::to_owned)
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

fn is_package_name(library: &str) -> bool {
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
    let docs = global_docs(same.iter().copied(), libraries);
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
