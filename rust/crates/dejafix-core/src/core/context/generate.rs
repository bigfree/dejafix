use jiff::SignedDuration;
use jiff::tz::TimeZone;

use super::sections::{arch_context, decisions, file_gotchas, fixes, length, located, warnings};
use super::types::{ContextInput, Level, Scored, Signals};
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::log::EventLog;

const CHARS_PER_TOKEN: i64 = 4;
const DAY_SECONDS: i64 = 86_400;
const FALLBACK_AGE_DAYS: i64 = 15;
const DECISIONS_GATE: f64 = 0.6;
const FIXES_GATE: f64 = 0.8;
const GOTCHAS_GATE: f64 = 0.9;
const ARCH_GATE: f64 = 0.95;

impl Level {
    fn for_budget(budget: u32) -> Self {
        match budget {
            4000.. => Self::Full,
            1000.. => Self::Compressed,
            200.. => Self::Ultra,
            _ => Self::Emergency,
        }
    }

    pub(super) fn is_tight(self) -> bool {
        matches!(self, Self::Ultra | Self::Emergency)
    }

    pub(super) fn is_roomy(self) -> bool {
        matches!(self, Self::Full | Self::Compressed)
    }
}

#[must_use]
pub fn generate_context(log: &EventLog<'_>, input: &ContextInput<'_>) -> String {
    let focus = input.focus.filter(|focus| !focus.is_empty());
    let signals = signals(input, focus);
    let mut scored: Vec<Scored<'_, '_>> = log
        .events()
        .iter()
        .filter(|event| !log.is_retired(&event.id))
        .filter_map(|event| {
            let score = score_event(event, &signals);
            (score > 0.0).then_some(Scored { score, event })
        })
        .collect();
    scored.sort_by(|left, right| right.score.total_cmp(&left.score));

    let level = Level::for_budget(input.budget);
    let char_budget = i64::from(input.budget) * CHARS_PER_TOKEN;
    let header = format!("## dejafix context (budget: {} tokens)\n", input.budget);
    let mut used = length(&header);
    let mut sections = vec![header];
    let mut admit = |section: String, used: &mut i64| {
        if !section.is_empty() {
            *used += length(&section);
            sections.push(section);
        }
    };

    admit(warnings(&scored, level, input.now), &mut used);
    if below(input.budget, used, DECISIONS_GATE) {
        admit(
            decisions(&scored, level, char_budget - used, input.now),
            &mut used,
        );
    }
    if below(input.budget, used, FIXES_GATE) {
        admit(
            fixes(&scored, level, char_budget - used, input.now),
            &mut used,
        );
    }
    if below(input.budget, used, GOTCHAS_GATE) {
        admit(
            file_gotchas(&scored, focus, level, char_budget - used),
            &mut used,
        );
    }
    if below(input.budget, used, ARCH_GATE)
        && level.is_roomy()
        && let Some(map) = input.project_map
    {
        admit(arch_context(map, focus, char_budget - used), &mut used);
    }
    sections.join("\n")
}

fn below(budget: u32, used: i64, fraction: f64) -> bool {
    let used = u32::try_from(used).unwrap_or(u32::MAX);
    f64::from(budget) * 4.0 * fraction > f64::from(used)
}

fn signals<'s>(input: &'s ContextInput<'s>, focus: Option<&'s str>) -> Signals<'s> {
    let window = SignedDuration::from_secs(input.recent_days.saturating_mul(DAY_SECONDS));
    Signals {
        now: input.now,
        cutoff: input
            .now
            .checked_sub(window)
            .unwrap_or(jiff::Timestamp::MIN),
        fallback: input
            .now
            .checked_sub(SignedDuration::from_secs(FALLBACK_AGE_DAYS * DAY_SECONDS))
            .unwrap_or(input.now),
        focus,
        git: input.git_files.iter().map(String::as_str).collect(),
    }
}

fn score_event(event: &Event<'_>, signals: &Signals<'_>) -> f64 {
    let at = event
        .at
        .utc()
        .and_then(|at| at.to_zoned(TimeZone::UTC).ok())
        .map_or(signals.fallback, |at| at.timestamp());
    let failed_attempt = event.kind == EventKind::Attempt && event.outcome == Some(Outcome::Failed);
    if at < signals.cutoff && !failed_attempt {
        return 0.0;
    }
    let base = match event.kind {
        EventKind::Attempt => 8.0,
        EventKind::Fix => 6.0,
        EventKind::Decision => 5.0,
        EventKind::Issue => 4.0,
        EventKind::Hypothesis => 3.0,
        EventKind::Note => 2.0,
    };
    let outcome = match event.outcome {
        Some(Outcome::Failed) => 2.0,
        Some(Outcome::Worked) => 0.8,
        Some(Outcome::Partial) => 1.2,
        None => 1.0,
    };
    let age_days = signals.now.duration_since(at).as_secs_f64() / 86_400.0;
    let recency = if age_days <= 1.0 {
        1.0
    } else if age_days <= 7.0 {
        0.8
    } else if age_days <= 30.0 {
        0.5
    } else {
        0.2
    };
    let boost = if failed_attempt { 1.5 } else { 1.0 };
    base * outcome * recency * relevance(event, signals) * boost
}

fn relevance(event: &Event<'_>, signals: &Signals<'_>) -> f64 {
    let located = located(event);
    let files = || {
        event
            .files
            .iter()
            .map(AsRef::as_ref)
            .chain(located.as_deref())
    };
    let mut relevance = 0.3;
    if let Some(focus) = signals.focus {
        let directory = focus.trim_end_matches('/');
        if files().any(|file| file.starts_with(focus) || file.contains(focus)) {
            relevance = 1.0;
        } else if focus.contains('/')
            && files().any(|file| {
                file.rsplit_once('/')
                    .is_some_and(|(parent, _)| parent == directory)
            })
        {
            relevance = 0.7;
        }
    }
    if files().any(|file| signals.git.contains(file)) {
        relevance = f64::max(relevance, 0.9);
    }
    relevance
}
