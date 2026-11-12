use super::{
    is_python_decimal, is_python_space, python_lines, python_strip, python_strip_matches, zfill,
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
