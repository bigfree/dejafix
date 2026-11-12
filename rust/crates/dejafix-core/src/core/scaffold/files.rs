use crate::core::text::{is_python_space, python_lines, python_strip};

const GITIGNORE_ENTRIES: [&str; 4] = [
    ".dejafix/events.jsonl",
    ".dejafix/watch.pid",
    ".dejafix/watch.log",
    ".dejafix/structure.json",
];
const IDEAS_HEADING: &str = "## Ideas\n";

#[must_use]
pub fn gitignore_with(existing: &str, extra: &[&str]) -> Option<String> {
    let lines: Vec<&[u8]> = python_lines(existing.as_bytes())
        .map(|(_, line)| line)
        .collect();
    let fresh: Vec<&str> = GITIGNORE_ENTRIES
        .iter()
        .chain(extra)
        .copied()
        .filter(|entry| !lines.contains(&entry.as_bytes()))
        .collect();
    if fresh.is_empty() {
        return None;
    }
    let separator = match lines.last() {
        None | Some([]) => "",
        Some(_) => "\n",
    };
    Some(format!("{existing}{separator}{}\n", fresh.join("\n")))
}

#[must_use]
pub fn plan_with_idea(existing: &str, idea: &str) -> String {
    let line = format!("- {}\n", python_strip(idea));
    if existing.contains(IDEAS_HEADING) {
        return existing.replacen(IDEAS_HEADING, &format!("{IDEAS_HEADING}{line}"), 1);
    }
    format!(
        "{}\n\n{IDEAS_HEADING}{line}",
        existing.trim_end_matches(is_python_space)
    )
}
