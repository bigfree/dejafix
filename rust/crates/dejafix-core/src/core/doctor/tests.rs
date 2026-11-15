use serde_json::{Map, Value, json};

use super::{Reverted, SeenConfig, is_newer, track_pins};

fn seen(client: &'static str, key: &str, pinned: bool) -> SeenConfig<'static> {
    SeenConfig {
        client,
        key: key.to_owned(),
        pinned,
    }
}

const NONE: [&str; 0] = [];

fn clients(reverted: &[Reverted]) -> Vec<&str> {
    reverted.iter().map(|entry| entry.client.as_str()).collect()
}

#[test]
fn a_config_that_was_clean_and_is_pinned_again_is_reported() {
    let mut state = Map::new();
    assert_eq!(
        clients(&track_pins(
            &mut state,
            &[seen("Cursor", "/c", true)],
            "2026-11-12 10:00"
        )),
        NONE
    );
    assert_eq!(
        clients(&track_pins(
            &mut state,
            &[seen("Cursor", "/c", false)],
            "2026-11-12 10:05"
        )),
        NONE
    );
    let reverted = track_pins(
        &mut state,
        &[seen("Cursor", "/c", true)],
        "2026-11-12 10:09",
    );
    assert_eq!(
        reverted,
        [Reverted {
            client: "Cursor".to_owned(),
            clean_since: "2026-11-12 10:05".to_owned()
        }]
    );
}

#[test]
fn a_config_pinned_all_along_is_not_a_revert() {
    let mut state = Map::new();
    for _ in 0..2 {
        assert_eq!(
            clients(&track_pins(
                &mut state,
                &[seen("Cursor", "/c", true)],
                "now"
            )),
            NONE
        );
    }
}

#[test]
fn a_clean_config_is_recorded_the_first_time_it_is_seen() {
    let mut state = Map::new();
    assert_eq!(
        clients(&track_pins(
            &mut state,
            &[seen("Cursor", "/c", false)],
            "then"
        )),
        NONE
    );
    assert_eq!(
        state.get("/c"),
        Some(&json!({"client": "Cursor", "pinned": false, "seen": "then"}))
    );
    let reverted = track_pins(&mut state, &[seen("Cursor", "/c", true)], "now");
    assert_eq!(clients(&reverted), ["Cursor"]);
}

#[test]
fn nothing_seen_records_nothing() {
    let mut state = Map::new();
    assert_eq!(clients(&track_pins(&mut state, &[], "now")), NONE);
    assert!(state.is_empty());
}

#[test]
fn a_path_seen_twice_keeps_the_last_client_and_wins_over_clean() {
    let mut state = Map::new();
    state.insert("/c".to_owned(), json!({"pinned": false}));
    let configs = [
        seen("Cursor", "/c", true),
        seen("This project (.cursor/mcp.json)", "/c", true),
        seen("Codex", "/c", false),
    ];
    let reverted = track_pins(&mut state, &configs, "now");
    assert_eq!(
        reverted,
        [Reverted {
            client: "This project (.cursor/mcp.json)".to_owned(),
            clean_since: "a previous run".to_owned()
        }]
    );
    assert_eq!(
        state.get("/c").and_then(|entry| entry.get("pinned")),
        Some(&Value::Bool(true))
    );
}

#[test]
fn odd_previous_entries_are_read_like_python() {
    let mut state = Map::new();
    state.insert("/empty".to_owned(), json!({}));
    state.insert("/list".to_owned(), json!([1]));
    state.insert("/stamp".to_owned(), json!({"pinned": false, "seen": null}));
    let configs = [
        seen("A", "/empty", true),
        seen("B", "/list", true),
        seen("C", "/stamp", true),
    ];
    let reverted = track_pins(&mut state, &configs, "now");
    assert_eq!(
        reverted,
        [Reverted {
            client: "C".to_owned(),
            clean_since: "None".to_owned()
        }]
    );
}

#[test]
fn a_higher_release_is_newer() {
    assert!(is_newer("0.3.1", "0.3.0"));
    assert!(is_newer("0.10.0", "0.9.9"));
    assert!(!is_newer("0.3.0", "0.3.1"));
    assert!(!is_newer("0.3.0", "0.3.0"));
}

#[test]
fn versions_follow_the_pep_440_order() {
    let ordered = [
        "1.dev0",
        "1.0.dev456",
        "1.0a1",
        "1.0a2.dev456",
        "1.0a12.dev456",
        "1.0a12",
        "1.0b1.dev456",
        "1.0b2",
        "1.0b2.post345.dev456",
        "1.0b2.post345",
        "1.0rc1.dev456",
        "1.0rc1",
        "1.0",
        "1.0.post456.dev34",
        "1.0.post456",
        "1.0.15",
        "1.1.dev1",
    ];
    for pair in ordered.windows(2) {
        assert!(is_newer(pair[1], pair[0]), "{} > {}", pair[1], pair[0]);
        assert!(!is_newer(pair[0], pair[1]), "{} < {}", pair[0], pair[1]);
    }
}

#[test]
fn cargo_and_pep_440_spellings_of_a_candidate_are_equal() {
    assert!(!is_newer("1.0.0-rc.3", "1.0.0rc3"));
    assert!(!is_newer("1.0.0rc3", "1.0.0-rc.3"));
    assert!(!is_newer("1.0.0RC3", "1.0.0rc3"));
    assert!(!is_newer("1.0.0c3", "1.0.0rc3"));
    assert!(!is_newer("v1.0.0", "1.0.0"));
}

#[test]
fn a_final_release_is_newer_than_its_candidates() {
    assert!(is_newer("1.0.0", "1.0.0-rc.3"));
    assert!(!is_newer("1.0.0rc3", "1.0.0"));
    assert!(is_newer("1.0.0rc4", "1.0.0-rc.3"));
    assert!(is_newer("1.0.0-beta.1", "1.0.0-alpha.2"));
}

#[test]
fn trailing_zeros_do_not_change_a_version() {
    assert!(!is_newer("1.0.0", "1.0"));
    assert!(!is_newer("1.0", "1.0.0"));
    assert!(is_newer("1.0.1", "1.0"));
}

#[test]
fn an_unreadable_version_is_never_newer_and_nothing_is_newer_than_it() {
    for unreadable in [
        "",
        "abc",
        "x.y.z",
        "1.0+local",
        "1!1.0",
        "1.0.0-beta-two",
        "99999999999999999999999.1",
    ] {
        assert!(!is_newer(unreadable, "1.0.0"), "{unreadable}");
        assert!(!is_newer("9.0.0", unreadable), "{unreadable}");
    }
}
