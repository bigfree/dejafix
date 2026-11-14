use std::borrow::Cow;

use super::{Analyzer, STOP_WORDS, stem};

const VOCABULARY: &str = include_str!("../../../tests/data/snowball-english/voc.txt");
const STEMS: &str = include_str!("../../../tests/data/snowball-english/output.txt");

fn analyzed(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    Analyzer.terms(text, &mut out);
    out.into_iter().map(Cow::into_owned).collect()
}

#[test]
fn the_snowball_sample_stems_exactly() {
    let pairs: Vec<(&str, &str)> = VOCABULARY.lines().zip(STEMS.lines()).collect();
    let mismatches: Vec<String> = pairs
        .iter()
        .filter(|(word, expected)| stem(word) != *expected)
        .map(|(word, expected)| format!("{word}: {} != {expected}", stem(word)))
        .collect();
    assert_eq!(pairs.len(), 4265);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn whole_word_exceptions_follow_the_definition() {
    for (word, expected) in [
        ("skis", "ski"),
        ("skies", "sky"),
        ("idly", "idl"),
        ("gently", "gentl"),
        ("ugly", "ugli"),
        ("early", "earli"),
        ("only", "onli"),
        ("singly", "singl"),
        ("sky", "sky"),
        ("news", "news"),
        ("howe", "howe"),
        ("atlas", "atlas"),
        ("cosmos", "cosmos"),
        ("bias", "bias"),
        ("andes", "andes"),
    ] {
        assert_eq!(stem(word), expected, "{word}");
    }
}

#[test]
fn ing_exceptions_follow_the_definition() {
    for (word, expected) in [
        ("dying", "die"),
        ("lying", "lie"),
        ("tying", "tie"),
        ("vying", "vie"),
        ("inning", "inning"),
        ("outing", "outing"),
        ("canning", "canning"),
        ("herring", "herring"),
        ("earring", "earring"),
        ("evening", "evening"),
        ("flying", "fli"),
    ] {
        assert_eq!(stem(word), expected, "{word}");
    }
}

#[test]
fn special_r1_prefixes_keep_their_stems_apart() {
    for (word, expected) in [
        ("generate", "generat"),
        ("general", "general"),
        ("communism", "communism"),
        ("arsenal", "arsenal"),
        ("pasted", "paste"),
        ("universal", "universal"),
        ("lateral", "lateral"),
        ("emergency", "emergenc"),
        ("emerge", "emerg"),
        ("organic", "organic"),
        ("international", "internat"),
    ] {
        assert_eq!(stem(word), expected, "{word}");
    }
}

#[test]
fn a_stem_that_only_drops_a_suffix_borrows_the_input() {
    assert!(matches!(stem("connected"), Cow::Borrowed("connect")));
    assert!(matches!(stem("config"), Cow::Borrowed("config")));
    assert!(matches!(stem("happy"), Cow::Owned(_)));
}

#[test]
fn words_the_stemmer_does_not_handle_are_returned_unchanged() {
    assert_eq!(stem("as"), "as");
    assert_eq!(stem("Running"), "Running");
    assert_eq!(stem("caf\u{e9}s"), "caf\u{e9}s");
    let long = "a".repeat(100);
    assert_eq!(stem(&long), long);
}

#[test]
fn identifiers_split_and_keep_the_whole_token() {
    assert_eq!(
        analyzed("ts.readConfigFile"),
        ["ts", "read", "config", "file", "readconfigfile"]
    );
    assert_eq!(analyzed("UTF-8"), ["utf", "8"]);
    assert_eq!(analyzed("snake_case_name"), ["snake", "case", "name"]);
    assert_eq!(analyzed("sha256sum"), ["sha", "256", "sum", "sha256sum"]);
}

#[test]
fn stop_words_and_single_letters_are_dropped() {
    assert_eq!(
        analyzed("The worker is not exiting on a SIGTERM"),
        ["worker", "exit", "sigterm"]
    );
    assert_eq!(analyzed("x y z"), Vec::<String>::new());
}

#[test]
fn empty_and_punctuation_only_texts_give_no_terms() {
    assert_eq!(analyzed(""), Vec::<String>::new());
    assert_eq!(analyzed("  -- ... !!! "), Vec::<String>::new());
}

#[test]
fn analyzed_terms_append_to_the_callers_buffer() {
    let mut out = vec![Cow::Borrowed("kept")];
    Analyzer.terms("redis connection", &mut out);
    assert_eq!(out, ["kept", "redi", "connect"]);
}

#[test]
fn lower_case_tokens_borrow_from_the_text() {
    let mut out = Vec::new();
    Analyzer.terms("redis Redis", &mut out);
    assert!(matches!(out.first(), Some(Cow::Borrowed("redi"))));
    assert!(matches!(out.get(1), Some(Cow::Owned(_))));
}

#[test]
fn stop_words_are_sorted_for_binary_search() {
    assert!(STOP_WORDS.windows(2).all(|pair| pair[0] < pair[1]));
}
