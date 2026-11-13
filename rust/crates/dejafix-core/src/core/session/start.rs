use crate::core::summary::looks_like_placeholder;
use crate::core::text::{python_lines, python_strip};

pub const SESSION_START_DONE: &str = "start_session() has already returned what get_instructions, get_summary and get_plan would at session start: the rules, the memory summary and the current plan follow. Where the rules below ask for those calls at session start, they are done.";
pub const MAP_POINTER: &str =
    "Project map: call get_project_map() only when the file layout matters.";

const SETTLED_PLAN_SECTIONS: [&str; 3] = ["Ideas", "Someday / maybe", "Shipped"];

fn heading_of(section: &str) -> &str {
    python_lines(section.as_bytes())
        .next()
        .and_then(|(_, line)| std::str::from_utf8(line).ok())
        .map_or("", python_strip)
}

#[must_use]
pub fn current_plan(plan: &str) -> String {
    let prefixed = format!("\n{plan}");
    prefixed
        .split("\n## ")
        .skip(1)
        .filter(|section| {
            let heading = heading_of(section);
            !heading.is_empty() && !SETTLED_PLAN_SECTIONS.contains(&heading)
        })
        .map(|section| format!("## {}", python_strip(section)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[must_use]
pub fn placeholder_map(map: &str) -> &str {
    if !map.is_empty() && looks_like_placeholder(map) {
        map
    } else {
        ""
    }
}

#[must_use]
pub fn session_start(instructions: &str, summary: &str, plan: &str, map: &str) -> String {
    let plan = current_plan(plan);
    [
        SESSION_START_DONE,
        instructions,
        summary,
        plan.as_str(),
        placeholder_map(map),
        MAP_POINTER,
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n")
}
