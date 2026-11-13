use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;

use jiff::SignedDuration;
use jiff::tz::TimeZone;

use super::types::{BriefInput, Codes};
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::global::is_truthy;
use crate::core::json::python_str;
use crate::core::path::location_to_file;
use crate::core::precheck::RECENT_DAYS;
use crate::core::score::{ScoreEvent, calculate_score, score_event_of};
use crate::core::stale::CommitsSince;
use crate::core::text::char_prefix;

const RULE: &str = "─";
const RULE_WIDTH: usize = 60;
const MAX_WARNINGS: usize = 4;
const MAX_STALE: usize = 5;
const MAX_ISSUES: usize = 4;
const MAX_DECISIONS: usize = 5;
const MAX_GOTCHAS: usize = 3;
const WEEK_HOURS: i64 = 24 * 7;
const NO_FILE: &str = "(no file)";

fn line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

fn instant(event: &Event<'_>) -> Option<jiff::Timestamp> {
    TimeZone::UTC.to_timestamp(event.at.utc()?).ok()
}

fn ago(now: jiff::Timestamp, hours: i64) -> jiff::Timestamp {
    now.checked_sub(SignedDuration::from_hours(hours))
        .unwrap_or(jiff::Timestamp::MIN)
}

fn file_of(event: &Event<'_>) -> String {
    let file = match event
        .location
        .as_deref()
        .filter(|location| !location.is_empty())
    {
        Some(location) => location_to_file(location)
            .map(std::borrow::Cow::into_owned)
            .filter(|file| !file.is_empty())
            .unwrap_or_else(|| location.to_owned()),
        None => event
            .files
            .first()
            .map(|file| file.as_ref().to_owned())
            .unwrap_or_default(),
    };
    if file.is_empty() {
        NO_FILE.to_owned()
    } else {
        file
    }
}

fn warnings(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    let cutoff = ago(input.now, 24 * i64::from(RECENT_DAYS));
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<(String, Vec<&Event<'_>>)> = Vec::new();
    for event in input.log.events() {
        if event.kind != EventKind::Attempt
            || event.outcome != Some(Outcome::Failed)
            || instant(event).is_none_or(|at| at < cutoff)
        {
            continue;
        }
        let file = file_of(event);
        if let Some(slot) = index.get(&file).and_then(|slot| groups.get_mut(*slot)) {
            slot.1.push(event);
        } else {
            index.insert(file.clone(), groups.len());
            groups.push((file, vec![event]));
        }
    }
    line(
        out,
        &format!("{}⚠ Active warnings{}", codes.yellow, codes.reset),
    );
    if groups.is_empty() {
        line(
            out,
            &format!(
                "   {}none — no failed attempts in {RECENT_DAYS} days{}",
                codes.dim, codes.reset
            ),
        );
        return;
    }
    groups.sort_by_key(|(_, items)| Reverse(items.len()));
    for (file, items) in groups.iter().take(MAX_WARNINGS) {
        let last = items.last().map_or("", |event| event.summary.as_str());
        let plural = if items.len() == 1 { "" } else { "s" };
        line(
            out,
            &format!(
                "   {file} — {} failed attempt{plural} {}(last: {}){}",
                items.len(),
                codes.dim,
                char_prefix(last, 60),
                codes.reset
            ),
        );
    }
}

fn stale(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    if input.stale.is_empty() {
        return;
    }
    line(
        out,
        &format!("{}⏳ Possibly stale{}", codes.yellow, codes.reset),
    );
    for item in input.stale.iter().take(MAX_STALE) {
        let reason = match item.commits_since {
            CommitsSince::Missing => format!("{} no longer exists", item.file),
            CommitsSince::Count(count) => format!("{} changed {count}x since", item.file),
        };
        line(
            out,
            &format!(
                "   {} [{}] {} {}— {reason}{}",
                item.event.kind.as_str(),
                item.event.id.as_str(),
                char_prefix(item.event.summary.as_str(), 55),
                codes.dim,
                codes.reset
            ),
        );
    }
    line(
        out,
        &format!(
            "   {}confirm, or retire: djf decision \"...\" --supersedes <id>{}",
            codes.dim, codes.reset
        ),
    );
}

fn open_issues(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    let open: Vec<&Event<'_>> = input
        .log
        .events()
        .iter()
        .filter(|event| {
            event.kind == EventKind::Issue
                && !event
                    .issue
                    .as_ref()
                    .is_some_and(|issue| input.log.is_closed(issue))
        })
        .collect();
    line(out, &format!("{}📋 Open issues{}", codes.red, codes.reset));
    if open.is_empty() {
        line(out, &format!("   {}none open{}", codes.dim, codes.reset));
        return;
    }
    for issue in open.iter().skip(open.len().saturating_sub(MAX_ISSUES)) {
        let id = issue.issue.as_ref().map_or("None", |issue| issue.as_str());
        line(
            out,
            &format!("   #{id} {}", char_prefix(issue.summary.as_str(), 70)),
        );
    }
}

fn decisions(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    let live: Vec<&Event<'_>> = input
        .log
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Decision && !input.log.is_retired(&event.id))
        .collect();
    line(
        out,
        &format!("{}🕑 Recent decisions{}", codes.teal, codes.reset),
    );
    if live.is_empty() {
        line(
            out,
            &format!("   {}none logged yet{}", codes.dim, codes.reset),
        );
        return;
    }
    for decision in live.iter().skip(live.len().saturating_sub(MAX_DECISIONS)) {
        line(
            out,
            &format!("   {}", char_prefix(decision.summary.as_str(), 75)),
        );
    }
}

fn gotchas(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    if input.gotchas.is_empty() {
        return;
    }
    line(
        out,
        &format!("{}💡 Stack gotchas{}", codes.teal, codes.reset),
    );
    let skip = input.gotchas.len().saturating_sub(MAX_GOTCHAS);
    for gotcha in input.gotchas.iter().skip(skip) {
        let library = gotcha.get("library").map(python_str).unwrap_or_default();
        let text = ["text", "summary"]
            .iter()
            .filter_map(|key| gotcha.get(*key))
            .find(|value| is_truthy(value))
            .map(python_str)
            .unwrap_or_default();
        line(out, &format!("   {library}: {}", char_prefix(&text, 70)));
    }
}

fn score(out: &mut String, input: &BriefInput<'_, '_>, codes: Codes) {
    let events: Vec<ScoreEvent<'_>> = input.log.events().iter().map(score_event_of).collect();
    let week_ago = ago(input.now, WEEK_HOURS);
    let older: Vec<ScoreEvent<'_>> = events
        .iter()
        .filter(|event| event.at.is_some_and(|at| at < week_ago))
        .cloned()
        .collect();
    let current = calculate_score(&events);
    let delta = i64::from(current.score) - i64::from(calculate_score(&older).score);
    let trend = match delta {
        1.. => format!(" {}▲ +{delta} this week{}", codes.green, codes.reset),
        0 => format!(" {}— unchanged this week{}", codes.dim, codes.reset),
        _ => format!(" {}▼ {delta} this week{}", codes.red, codes.reset),
    };
    line(
        out,
        &format!(
            "{}📈 Score{}  {} ({}/100){trend}",
            codes.green, codes.reset, current.grade, current.score
        ),
    );
}

#[must_use]
pub fn render_brief(input: &BriefInput<'_, '_>) -> String {
    let codes = if input.color {
        Codes::COLOR
    } else {
        Codes::PLAIN
    };
    let rule = format!("{}{}{}", codes.dim, RULE.repeat(RULE_WIDTH), codes.reset);
    let mut out = String::new();
    line(&mut out, "");
    let _ = writeln!(
        out,
        "{}dejafix brief — {}{}",
        codes.bold, input.name, codes.reset
    );
    line(&mut out, &rule);
    warnings(&mut out, input, codes);
    stale(&mut out, input, codes);
    open_issues(&mut out, input, codes);
    decisions(&mut out, input, codes);
    gotchas(&mut out, input, codes);
    score(&mut out, input, codes);
    line(&mut out, &rule);
    line(&mut out, "");
    out
}
