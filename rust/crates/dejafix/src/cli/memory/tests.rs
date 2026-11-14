use dejafix_core::GlobalAction;

use super::global::action_of;
use crate::cli::args::GlobalArgs;

fn args(action: &str, text: Option<&str>, library: Option<&str>) -> GlobalArgs {
    GlobalArgs {
        action: action.to_owned(),
        text: text.map(str::to_owned),
        tag: None,
        library: library.map(str::to_owned),
        older_than: 365,
        confidence: None,
        format: "text".to_owned(),
        keyword: Vec::new(),
        versions: None,
        budget: 2_000,
    }
}

#[test]
fn add_with_a_library_becomes_add_gotcha() {
    assert_eq!(
        action_of(args("add", Some("close sessions"), Some("sqlalchemy"))),
        GlobalAction::AddGotcha {
            library: Some("sqlalchemy".to_owned()),
            text: Some("close sessions".to_owned()),
            keywords: Vec::new(),
            versions: None,
        }
    );
}

#[test]
fn add_gotcha_carries_keywords_and_versions() {
    let mut asked = args("add-gotcha", Some("text"), Some("bullmq"));
    asked.keyword = vec!["shutdown".to_owned(), "worker close".to_owned()];
    asked.versions = Some(">=6".to_owned());
    assert_eq!(
        action_of(asked),
        GlobalAction::AddGotcha {
            library: Some("bullmq".to_owned()),
            text: Some("text".to_owned()),
            keywords: vec!["shutdown".to_owned(), "worker close".to_owned()],
            versions: Some(">=6".to_owned()),
        }
    );
}

#[test]
fn add_with_an_empty_library_stays_a_pattern() {
    assert_eq!(
        action_of(args("add", Some("x"), Some(""))),
        GlobalAction::Add {
            text: Some("x".to_owned()),
            tag: None,
        }
    );
}

#[test]
fn only_the_exact_format_json_is_json() {
    let mut json = args("list", None, None);
    json.format = "json".to_owned();
    assert!(matches!(
        action_of(json),
        GlobalAction::List { json: true, .. }
    ));
    let mut other = args("detect", None, None);
    other.format = "JSON".to_owned();
    assert_eq!(action_of(other), GlobalAction::Detect { json: false });
}

#[test]
fn an_unknown_action_is_kept_verbatim() {
    assert_eq!(
        action_of(args("frobnicate", None, None)),
        GlobalAction::Unknown("frobnicate".to_owned())
    );
}
