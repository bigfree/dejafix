use serde_json::value::RawValue;
use serde_json::{Value, json};

use super::{
    is_dropped_by_to_dict, is_python_truthy, write_canonical, write_python_json,
    write_python_json_indented,
};

fn line(value: &Value) -> String {
    let mut out = Vec::new();
    write_canonical(&mut out, value).unwrap();
    String::from_utf8(out).unwrap()
}

fn raw(text: &str) -> Box<RawValue> {
    RawValue::from_string(text.to_owned()).unwrap()
}

#[test]
fn plain_json_keeps_insertion_order() {
    let value = json!({"b": 1, "a": 2});
    let mut out = Vec::new();
    write_python_json(&mut out, &value).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), r#"{"b": 1, "a": 2}"#);
}

#[test]
fn json_sorts_keys_and_uses_python_separators() {
    let value = json!({"b": 1, "a": [1, 2], "c": {"z": true, "y": null}});
    assert_eq!(
        line(&value),
        r#"{"a": [1, 2], "b": 1, "c": {"y": null, "z": true}}"#
    );
}

#[test]
fn json_escapes_like_ensure_ascii() {
    let value = json!({"s": "é – 🚀 \u{7f} \u{1} \n\t\"\\/"});
    assert_eq!(
        line(&value),
        r#"{"s": "\u00e9 \u2013 \ud83d\ude80 \u007f \u0001 \n\t\"\\/"}"#
    );
}

#[test]
fn json_writes_floats_like_python_repr() {
    let cases = [
        (0.1, "0.1"),
        (100.0, "100.0"),
        (1e16, "1e+16"),
        (1e-5, "1e-05"),
        (1.5e-7, "1.5e-07"),
        (123_456_789_012_345.6, "123456789012345.6"),
        (-2.5e20, "-2.5e+20"),
        (1e15, "1000000000000000.0"),
        (0.0001, "0.0001"),
        (5e-324, "5e-324"),
        (f64::MAX, "1.7976931348623157e+308"),
        (-0.0, "-0.0"),
        (12.5, "12.5"),
    ];
    for (value, expected) in cases {
        assert_eq!(line(&json!([value])), format!("[{expected}]"), "{value:e}");
    }
}

#[test]
fn json_leaves_empty_containers_compact() {
    assert_eq!(line(&json!({"a": [], "b": {}})), r#"{"a": [], "b": {}}"#);
}

#[test]
fn canonical_sorts_nested_keys_and_escapes_like_python() {
    let value: Value =
        serde_json::from_str(r#"{"b": {"z": 1, "a": [true, null]}, "a": "\u00e9\u007f"}"#).unwrap();
    let mut out = Vec::new();
    write_canonical(&mut out, &value).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        r#"{"a": "\u00e9\u007f", "b": {"a": [true, null], "z": 1}}"#
    );
}

#[test]
fn to_dict_drops_none_empty_lists_false_and_zero() {
    for text in ["null", "false", "0", "0.0", "-0.0", "0e5", "[]", "[ ]"] {
        assert!(is_dropped_by_to_dict(&raw(text)), "{text}");
    }
    for text in ["true", "1", "\"\"", "\"x\"", "{}", "[0]", "-1e-300"] {
        assert!(!is_dropped_by_to_dict(&raw(text)), "{text}");
    }
}

#[test]
fn truthiness_follows_python() {
    for text in [
        "null", "false", "0", "-0.0", "\"\"", "[]", "[ ]", "{}", "{ \n}",
    ] {
        assert!(!is_python_truthy(&raw(text)), "{text}");
    }
    for text in ["true", "1", "\" \"", "\"x\"", "{\"a\": 1}", "[0]", "1e-300"] {
        assert!(is_python_truthy(&raw(text)), "{text}");
    }
}

fn indented(value: &Value, indent: usize, sort_keys: bool) -> String {
    let mut out = Vec::new();
    write_python_json_indented(&mut out, value, indent, sort_keys).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn indented_output_matches_python_json_dump() {
    let value: Value = serde_json::from_str(
        r#"{"schema_version": 1, "active_project": null, "by_path": {"/é": {"tags": [], "id": "x"}}, "t": 1.5e-7, "e": {}}"#,
    )
    .unwrap();
    assert_eq!(
        indented(&value, 1, false),
        "{\n \"schema_version\": 1,\n \"active_project\": null,\n \"by_path\": {\n  \"/\\u00e9\": {\n   \"tags\": [],\n   \"id\": \"x\"\n  }\n },\n \"t\": 1.5e-07,\n \"e\": {}\n}"
    );
    assert_eq!(
        indented(&json!({"libraries": ["b", "a"], "a": 1}), 2, true),
        "{\n  \"a\": 1,\n  \"libraries\": [\n    \"b\",\n    \"a\"\n  ]\n}"
    );
    assert_eq!(indented(&json!([]), 1, false), "[]");
    assert_eq!(indented(&json!(["/a"]), 1, false), "[\n \"/a\"\n]");
}
