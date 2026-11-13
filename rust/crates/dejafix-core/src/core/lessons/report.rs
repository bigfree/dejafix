use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use super::types::{Candidate, Found, LessonKind, Memory, Normalizer, Record};
use crate::core::path::{PathResolver, normpath};
use crate::core::text::{char_len, char_prefix, collapse_whitespace, is_python_word, python_strip};

const SHOWN: usize = 3;
const TEXT_CHARS: usize = 160;

static LINE_SUFFIX: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r":\d+(?::\d+)?\z").ok());

static SAFE_ID: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\A[0-9A-Za-z_.\-]{1,24}\z").ok());

impl<'r, 'a> Memory<'r, 'a> {
    pub fn new(records: &'r [Record<'a>], root: &str, resolver: &impl PathResolver) -> Self {
        let mut normalizer = Normalizer {
            root,
            prefix: format!("{root}/"),
            resolver,
            cache: HashMap::new(),
        };
        let closed: HashSet<&str> = records
            .iter()
            .filter(|record| record.kind == "fix")
            .map(|record| record.issue_id.as_ref())
            .collect();
        let retired: HashSet<&str> = records
            .iter()
            .filter_map(|record| record.supersedes.as_deref())
            .collect();
        let mut issue_files: HashMap<&str, HashSet<String>> = HashMap::new();
        for record in records.iter().filter(|record| record.kind == "issue") {
            issue_files.insert(record.issue_id.as_ref(), normalizer.files_of(record));
        }
        let candidates = records
            .iter()
            .filter_map(|record| {
                let kind = lesson_kind(record, &closed, &retired)?;
                let mut files = normalizer.files_of(record);
                if files.is_empty() {
                    files = issue_files
                        .get(record.issue_id.as_ref())
                        .cloned()
                        .unwrap_or_default();
                }
                let mention_text = if kind == LessonKind::Decision && !files.is_empty() {
                    ""
                } else {
                    record.summary.as_ref()
                };
                Some(Candidate {
                    kind,
                    record,
                    files,
                    mention_text,
                })
            })
            .collect();
        Self { candidates }
    }

    fn lessons(&self, relative: &str) -> Found<'r, 'a> {
        let mut found = Found::default();
        for candidate in &self.candidates {
            let hit = candidate.files.contains(relative)
                || (candidate.mention_text.contains(relative)
                    && mention(candidate.mention_text, relative));
            if !hit {
                continue;
            }
            match candidate.kind {
                LessonKind::Issue => found.issues.push(candidate.record),
                LessonKind::Attempt => found.attempts.push(candidate.record),
                LessonKind::Decision => found.decisions.push(candidate.record),
            }
        }
        found
    }

    #[must_use]
    pub fn report(&self, relative: &str) -> String {
        let found = self.lessons(relative);
        if found.issues.is_empty() && found.attempts.is_empty() && found.decisions.is_empty() {
            return String::new();
        }
        let mut lines = vec![format!("dejafix lessons for {relative}:")];
        if !found.issues.is_empty() {
            lines.push("  Open issues on this file:".to_owned());
            lines.extend(
                last(&found.issues)
                    .iter()
                    .map(|issue| format!("    - {}{}", issue_label(issue), clip(&issue.summary))),
            );
            lines.extend(overflow_line(&found.issues));
        }
        if !found.attempts.is_empty() {
            let failed = found
                .attempts
                .iter()
                .filter(|attempt| attempt.outcome == "failed")
                .count();
            lines.push(format!(
                "  Earlier attempts ({failed} failed, {} partial), newest first:",
                found.attempts.len() - failed
            ));
            lines.extend(last(&found.attempts).iter().rev().map(|attempt| {
                let mark = if attempt.outcome == "failed" {
                    "x"
                } else {
                    "~"
                };
                format!(
                    "    {mark} {}{}",
                    clip(&attempt.summary),
                    provenance(attempt, true)
                )
            }));
            lines.extend(overflow_line(&found.attempts));
        }
        if !found.decisions.is_empty() {
            lines.push("  Decisions on this file:".to_owned());
            lines.extend(last(&found.decisions).iter().map(|decision| {
                format!(
                    "    - {}{}",
                    clip(&decision.summary),
                    provenance(decision, false)
                )
            }));
            lines.extend(overflow_line(&found.decisions));
        }
        lines.join("\n")
    }
}

impl<'s, R: PathResolver> Normalizer<'_, 's, R> {
    fn files_of(&mut self, record: &'s Record<'_>) -> HashSet<String> {
        record
            .files
            .iter()
            .map(AsRef::as_ref)
            .chain(std::iter::once(record.location.as_ref()))
            .filter(|path| !python_strip(path).is_empty())
            .map(|path| self.normalized(path))
            .collect()
    }

    fn normalized(&mut self, path: &'s str) -> String {
        if let Some(found) = self.cache.get(path) {
            return found.clone();
        }
        let stripped = python_strip(path);
        let cleaned = LINE_SUFFIX
            .as_ref()
            .map_or(stripped.into(), |pattern| pattern.replace(stripped, ""))
            .replace('\\', "/");
        let normal = if cleaned.starts_with('/') {
            if let Some(rest) = cleaned.strip_prefix(self.prefix.as_str()) {
                rest.to_owned()
            } else {
                self.resolver
                    .resolve(&cleaned)
                    .and_then(|resolved| relative_to(&resolved, self.root))
                    .unwrap_or(cleaned)
            }
        } else {
            normpath(cleaned.into()).into_owned()
        };
        self.cache.insert(path, normal.clone());
        normal
    }
}

fn relative_to(path: &str, root: &str) -> Option<String> {
    if path == root {
        return Some(".".to_owned());
    }
    path.strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
        .map(str::to_owned)
}

fn lesson_kind(
    record: &Record<'_>,
    closed: &HashSet<&str>,
    retired: &HashSet<&str>,
) -> Option<LessonKind> {
    match record.kind.as_ref() {
        "issue" if !closed.contains(record.issue_id.as_ref()) => Some(LessonKind::Issue),
        "attempt" if matches!(record.outcome.as_ref(), "failed" | "partial") => {
            Some(LessonKind::Attempt)
        }
        "decision"
            if !retired.contains(record.id.as_ref())
                && !record.capture_source.starts_with("git_") =>
        {
            Some(LessonKind::Decision)
        }
        _ => None,
    }
}

fn last<'v, 'r, 'a>(entries: &'v [&'r Record<'a>]) -> &'v [&'r Record<'a>] {
    entries
        .get(entries.len().saturating_sub(SHOWN)..)
        .unwrap_or_default()
}

fn safe_id<'r>(record: &'r Record<'_>) -> &'r str {
    let issue = record.issue_id.as_ref();
    if SAFE_ID
        .as_ref()
        .is_some_and(|pattern| pattern.is_match(issue))
    {
        issue
    } else {
        ""
    }
}

fn issue_label(record: &Record<'_>) -> String {
    let issue = safe_id(record);
    if issue.is_empty() {
        String::new()
    } else {
        format!("#{issue} ")
    }
}

fn provenance(record: &Record<'_>, with_issue: bool) -> String {
    let issue = if with_issue { safe_id(record) } else { "" };
    let date = char_prefix(&record.timestamp, 10);
    let issue = if issue.is_empty() {
        String::new()
    } else {
        format!("#{issue}")
    };
    let shown: Vec<&str> = [issue.as_str(), date]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    if shown.is_empty() {
        String::new()
    } else {
        format!(" ({})", shown.join(", "))
    }
}

fn overflow_line(entries: &[&Record<'_>]) -> Option<String> {
    let hidden = entries
        .get(..entries.len().saturating_sub(SHOWN))
        .unwrap_or_default();
    if hidden.is_empty() {
        return None;
    }
    let mut seen = HashSet::new();
    let ids: Vec<String> = hidden
        .iter()
        .rev()
        .map(|record| safe_id(record))
        .filter(|issue| seen.insert(*issue) && !issue.is_empty())
        .map(|issue| format!("#{issue}"))
        .collect();
    let source = if ids.is_empty() {
        String::new()
    } else {
        format!(", from {} (get_issue)", ids.join(", "))
    };
    Some(format!("    ... and {} more{source}", hidden.len()))
}

fn clip(text: &str) -> String {
    let flat = collapse_whitespace(text);
    if char_len(&flat) <= TEXT_CHARS {
        return flat.into_owned();
    }
    let head = char_prefix(&flat, TEXT_CHARS - 3);
    format!(
        "{}...",
        head.trim_end_matches(crate::core::text::is_python_space)
    )
}

#[must_use]
pub fn mention(text: &str, relative: &str) -> bool {
    let mut start = 0;
    while let Some(found) = text.get(start..).and_then(|rest| rest.find(relative)) {
        let at = start + found;
        if boundary_ok(text, at, at + relative.len()) {
            return true;
        }
        start = at
            + text
                .get(at..)
                .and_then(|rest| rest.chars().next())
                .map_or(1, char::len_utf8);
    }
    false
}

fn boundary_ok(text: &str, start: usize, end: usize) -> bool {
    let before = text.get(..start).and_then(|head| head.chars().next_back());
    if before.is_some_and(|character| {
        is_python_word(character) || matches!(character, '.' | '/' | '\\' | '-')
    }) {
        return false;
    }
    let mut after = text.get(end..).unwrap_or_default().chars();
    match after.next() {
        Some(character) if is_python_word(character) || matches!(character, '/' | '\\' | '-') => {
            false
        }
        Some('.') => !after.next().is_some_and(is_python_word),
        _ => true,
    }
}
