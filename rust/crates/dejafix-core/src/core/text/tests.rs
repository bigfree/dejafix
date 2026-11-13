use std::borrow::Cow;

use super::{
    casefold, char_len, char_prefix, collapse_whitespace, contains_word, decimal_value,
    is_python_decimal, is_python_space, is_python_word, python_int, python_lines, python_split,
    python_strip, python_strip_matches, universal_newlines, zfill,
};

fn lines(bytes: &[u8]) -> Vec<(usize, Vec<u8>)> {
    python_lines(bytes).map(|(n, l)| (n, l.to_vec())).collect()
}

#[test]
fn python_whitespace_includes_the_information_separators() {
    for c in [
        '\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{85}', '\u{a0}', '\u{3000}', '\t',
    ] {
        assert!(is_python_space(c), "{c:?}");
    }
    assert!(!is_python_space('\u{200b}'));
    assert!(!is_python_space('\u{feff}'));
    assert_eq!(python_strip("\u{1f} x \u{2028}"), "x");
    assert_eq!(python_strip_matches("\"'x'\"", &['"', '\'']), "x");
}

#[test]
fn lines_split_like_str_splitlines() {
    let text = "a\r\nb\rc\u{0b}d\u{0c}e\u{1c}f\u{85}g\u{2028}h\u{2029}i\u{1d}j\u{1e}k\n";
    let found: Vec<_> = python_lines(text.as_bytes())
        .map(|(n, l)| (n, std::str::from_utf8(l).unwrap()))
        .collect();
    assert_eq!(
        found,
        [
            (1, "a"),
            (2, "b"),
            (3, "c"),
            (4, "d"),
            (5, "e"),
            (6, "f"),
            (7, "g"),
            (8, "h"),
            (9, "i"),
            (10, "j"),
            (11, "k")
        ]
    );
}

#[test]
fn empty_lines_are_numbered_and_no_line_follows_the_last_separator() {
    assert_eq!(lines(b""), []);
    assert_eq!(lines(b"\n"), [(1, vec![])]);
    assert_eq!(
        lines(b"a\n\nb"),
        [(1, b"a".to_vec()), (2, vec![]), (3, b"b".to_vec())]
    );
    assert_eq!(lines(b"\r\r\n"), [(1, vec![]), (2, vec![])]);
}

#[test]
fn a_line_with_invalid_utf8_is_returned_raw_and_does_not_shift_numbers() {
    assert_eq!(
        lines(b"a\n\xff\xfe\nb"),
        [
            (1, b"a".to_vec()),
            (2, b"\xff\xfe".to_vec()),
            (3, b"b".to_vec())
        ]
    );
}

#[test]
fn decimal_and_zfill_follow_python() {
    assert!(is_python_decimal("0012") && is_python_decimal("١٢"));
    assert!(!is_python_decimal(""));
    assert!(!is_python_decimal("²"));
    assert!(!is_python_decimal("1a"));
    assert!(!is_python_decimal("1\n"));
    assert_eq!(zfill("7", 4), "0007");
    assert_eq!(zfill("١", 4), "000١");
    assert_eq!(zfill("12345", 4), "12345");
}

#[test]
fn casefold_matches_python_on_known_cases() {
    assert_eq!(casefold("Straße"), "strasse");
    assert_eq!(casefold("ﬁle"), "file");
    assert_eq!(casefold("ΣΑΣ"), "σασ");
    assert_eq!(casefold("İ"), "i\u{307}");
    assert!(matches!(casefold("plain"), Cow::Borrowed("plain")));
}

#[test]
fn split_and_collapse_use_python_whitespace() {
    assert_eq!(
        python_split(" a\u{1c}b\u{2028}c  ").collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(collapse_whitespace("a \n\t b"), "a b");
    assert_eq!(collapse_whitespace(" a"), "a");
    assert_eq!(collapse_whitespace("a  b"), "a b");
    assert_eq!(collapse_whitespace(""), "");
    assert!(matches!(
        collapse_whitespace("a b c"),
        Cow::Borrowed("a b c")
    ));
    assert_eq!(char_prefix("žluťoučký", 4), "žluť");
    assert_eq!(char_prefix("ab", 9), "ab");
    assert_eq!(char_len("žluť"), 4);
}

#[test]
fn decimal_keys_order_like_python_int() {
    let key = |text| decimal_value(text).unwrap();
    assert!(key("10000") > key("9999"));
    assert_eq!(key("0042"), key("42"));
    assert_eq!(key("\u{661}\u{662}"), key("12"));
    assert_eq!(key("\u{1d7d9}"), key("1"));
    assert!(key("0") < key("1"));
    assert_eq!(decimal_value("12a"), None);
    assert_eq!(decimal_value(""), None);
}

#[test]
fn python_word_characters() {
    for c in ['a', 'Ž', '\u{663}', '²', '_', '一', '7'] {
        assert!(is_python_word(c), "{c}");
    }
    for c in ['-', '.', ' ', '\u{301}', '‿', '\u{93e}'] {
        assert!(!is_python_word(c), "{c}");
    }
}

#[test]
fn decimal_successor_pads_and_carries() {
    assert_eq!(decimal_value("0009").unwrap().successor(4), "0010");
    assert_eq!(decimal_value("9999").unwrap().successor(4), "10000");
    assert_eq!(decimal_value("0").unwrap().successor(4), "0001");
    assert_eq!(
        decimal_value("99999999999999999999999999999999999999999")
            .unwrap()
            .successor(4),
        "100000000000000000000000000000000000000000"
    );
}

#[test]
fn universal_newlines_follow_python_text_mode() {
    assert_eq!(universal_newlines("a\r\nb\rc\n"), "a\nb\nc\n");
    assert!(matches!(universal_newlines("a\nb"), Cow::Borrowed(_)));
}

#[test]
fn contains_word_uses_python_word_boundaries() {
    assert!(contains_word("use react here", "react"));
    assert!(!contains_word("imagineering", "gin"));
    assert!(contains_word("react-dom", "react"));
    assert!(!contains_word("x @types/node y", "@types/node"));
    assert!(contains_word("a@types/node", "@types/node"));
    assert!(!contains_word("reactive", "react"));
    assert!(!contains_word("préact", "act"));
    assert!(!contains_word("", "x"));
    assert!(!contains_word("x", ""));
}

#[test]
fn integers_parse_like_python_int() {
    for (text, value) in [
        ("42", 42),
        (" +42\n", 42),
        ("-7", -7),
        ("1_000", 1000),
        ("\u{ff15}", 5),
        ("\u{661}\u{662}", 12),
        ("0", 0),
    ] {
        assert_eq!(python_int(text), Some(value), "{text:?}");
    }
    for text in [
        "",
        "+",
        "-",
        "++5",
        "_1",
        "1_",
        "1__0",
        "1.0",
        "0x1",
        "99999999999999999999",
    ] {
        assert_eq!(python_int(text), None, "{text:?}");
    }
}
