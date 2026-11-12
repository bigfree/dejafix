use dejafix_core::EditHookRun;

use super::call_record;

#[test]
fn the_call_record_counts_reports_without_their_text() {
    let run = EditHookRun {
        output: "{\"x\": \"dejafix lessons for a.py: ... dejafix lessons for b.py\"}".to_owned(),
        event: String::new(),
        tool: "apply_patch".to_owned(),
        session: "s1".to_owned(),
        agent: String::new(),
        cwd: "/r".to_owned(),
        files: 2,
        error: String::new(),
    };
    assert_eq!(
        call_record(&run, "2027-01-15T09:00:00+0100", 3.04),
        r#"{"at": "2027-01-15T09:00:00+0100", "event": "", "tool": "apply_patch", "session": "s1", "agent": "", "cwd": "/r", "files": 2, "reported": 2, "chars": 63, "ms": 3.0, "error": ""}"#
    );
}
