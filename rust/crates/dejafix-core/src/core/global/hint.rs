use super::types::GlobalHint;
use crate::core::event::{EventKind, Outcome};
use crate::core::text::{contains_word, is_python_space};

const LESSON_PREFIXES: [&str; 10] = [
    "gotcha:", "lesson:", "warning:", "caution:", "pitfall:", "avoid:", "don't ", "do not ",
    "never ", "bug:",
];

const ANY_LIBRARY: &str = "<library>";

#[must_use]
pub fn hint_candidate(summary: &str, kind: EventKind, outcome: Option<Outcome>) -> GlobalHint {
    match kind {
        EventKind::Attempt if matches!(outcome, Some(Outcome::Failed | Outcome::Partial)) => {
            GlobalHint::NamedLibrary
        }
        EventKind::Decision | EventKind::Note => {
            let head = summary.trim_start_matches(is_python_space).to_lowercase();
            if LESSON_PREFIXES
                .iter()
                .any(|prefix| head.starts_with(prefix))
            {
                GlobalHint::AnyLibrary
            } else {
                GlobalHint::None
            }
        }
        _ => GlobalHint::None,
    }
}

#[must_use]
pub fn global_hint(summary: &str, candidate: GlobalHint, libraries: &[String]) -> Option<String> {
    if candidate == GlobalHint::None {
        return None;
    }
    let lowered = summary.to_lowercase();
    let named = libraries
        .iter()
        .map(|library| library.to_lowercase())
        .filter(|library| !library.is_empty() && contains_word(&lowered, library))
        .max_by_key(String::len);
    let library = match (named, candidate) {
        (Some(library), _) => library,
        (None, GlobalHint::AnyLibrary) => ANY_LIBRARY.to_owned(),
        (None, _) => return None,
    };
    Some(format!(
        "If this applies beyond this project, also call add_global_gotcha(library=\"{library}\", ...) in English without project details."
    ))
}
