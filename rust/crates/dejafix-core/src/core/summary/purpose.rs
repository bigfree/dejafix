use crate::core::text::{is_python_space, python_strip};

const PLACEHOLDER_PHRASES: [&str; 5] = [
    "Not described yet.",
    "Short description of what the project does.",
    "Replace this placeholder",
    "Status: not created yet",
    "This file should be created by the first AI assistant",
];

#[must_use]
pub fn looks_like_placeholder(text: &str) -> bool {
    let stripped = python_strip(text);
    stripped.is_empty()
        || PLACEHOLDER_PHRASES
            .iter()
            .any(|phrase| stripped.contains(phrase))
}

fn line_starts<'t>(text: &'t str, heading: &'t str) -> impl Iterator<Item = usize> + 't {
    text.match_indices(heading)
        .map(|(at, _)| at)
        .filter(|at| *at == 0 || text.get(..*at).is_some_and(|head| head.ends_with('\n')))
}

fn body_from(text: &str, start: usize) -> Option<&str> {
    let rest = text.get(start..)?;
    let end = rest.find("\n## ").unwrap_or(rest.len());
    let body = python_strip(rest.get(..end)?);
    (!looks_like_placeholder(body)).then_some(body)
}

#[must_use]
pub fn extract_purpose_from_map(map: &str) -> Option<&str> {
    const HEADING: &str = "## Project purpose";
    for at in line_starts(map, HEADING) {
        let after = at + HEADING.len();
        let rest = map.get(after..)?;
        let run = rest
            .char_indices()
            .find(|(_, character)| !is_python_space(*character))
            .map_or(rest.len(), |(index, _)| index);
        if let Some(newline) = rest.get(..run).and_then(|space| space.rfind('\n')) {
            return body_from(map, after + newline + 1);
        }
    }
    None
}

#[must_use]
pub fn extract_purpose_from_summary(summary: &str) -> Option<&str> {
    let mut starts: Vec<(usize, usize)> = ["## Project purpose\n", "## What this project is\n"]
        .into_iter()
        .flat_map(|heading| line_starts(summary, heading).map(move |at| (at, heading.len())))
        .collect();
    starts.sort_unstable();
    let (at, length) = starts.first()?;
    body_from(summary, at + length)
}
