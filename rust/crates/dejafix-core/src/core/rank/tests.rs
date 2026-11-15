use std::borrow::Cow;

use proptest::prelude::*;

use super::{Analyzer, Document, FieldSpec, Index, RankParams, STOP_WORDS, Scored, stem};

struct Doc {
    id: &'static str,
    fields: Vec<Vec<&'static str>>,
}

impl Document for Doc {
    fn id(&self) -> &str {
        self.id
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        for text in self.fields.get(field).into_iter().flatten() {
            visit(text);
        }
    }
}

const TITLE_FIRST: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        weight: 2.0,
    },
    FieldSpec {
        name: "body",
        weight: 1.0,
    },
];

const BODY_FIRST: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        weight: 1.0,
    },
    FieldSpec {
        name: "body",
        weight: 3.0,
    },
];

const BODY_HEAVY: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        weight: 1.0,
    },
    FieldSpec {
        name: "body",
        weight: 4.0,
    },
];

const ALL: RankParams = RankParams {
    min_coverage: 0.0,
    min_ratio: 0.0,
    ..RankParams::DEFAULT
};

fn doc(id: &'static str, title: &'static str, body: &'static str) -> Doc {
    Doc {
        id,
        fields: vec![vec![title], vec![body]],
    }
}

fn corpus() -> Vec<Doc> {
    vec![
        doc("c", "worker sigterm", "worker close timeout"),
        doc("a", "redis timeout", "worker"),
        doc("b", "queue", "redis redis worker"),
    ]
}

fn ranked(index: &Index, query: &str, params: &RankParams) -> Vec<(u32, String)> {
    index
        .rank(query, params)
        .iter()
        .map(|scored| (scored.doc, format!("{:.4}", scored.score)))
        .collect()
}

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

#[test]
fn bm25f_scores_match_the_hand_computed_values() {
    let index = Index::build(TITLE_FIRST, &corpus());
    assert_eq!(
        ranked(&index, "worker timeout", &ALL),
        [
            (1, "0.7861".to_owned()),
            (0, "0.6209".to_owned()),
            (2, "0.1196".to_owned())
        ]
    );
    assert_eq!(
        ranked(&index, "redis", &ALL),
        [(1, "0.6118".to_owned()), (2, "0.5982".to_owned())]
    );
}

#[test]
fn field_weights_change_the_order() {
    let body_first = Index::build(BODY_FIRST, &corpus());
    assert_eq!(
        ranked(&body_first, "worker timeout", &ALL),
        [
            (0, "0.9121".to_owned()),
            (1, "0.6736".to_owned()),
            (2, "0.1977".to_owned())
        ]
    );
    let body_heavy = Index::build(BODY_HEAVY, &corpus());
    assert_eq!(
        ranked(&body_heavy, "redis", &ALL),
        [(2, "0.8747".to_owned()), (1, "0.4345".to_owned())]
    );
}

#[test]
fn coverage_and_ratio_cut_irrelevant_documents() {
    let index = Index::build(TITLE_FIRST, &corpus());
    let docs = |query: &str, params: &RankParams| -> Vec<u32> {
        index
            .rank(query, params)
            .iter()
            .map(|scored| scored.doc)
            .collect()
    };
    assert_eq!(docs("worker timeout", &RankParams::DEFAULT), [1, 0]);
    let coverage_only = RankParams {
        min_ratio: 0.0,
        ..RankParams::DEFAULT
    };
    assert_eq!(docs("worker timeout", &coverage_only), [1, 0, 2]);
    assert_eq!(docs("sigterm queue close", &coverage_only), [0]);
    let strict = RankParams {
        min_coverage: 1.0,
        ..ALL
    };
    assert_eq!(docs("worker timeout", &strict), [1, 0]);
    let limited = RankParams { limit: 1, ..ALL };
    assert_eq!(docs("worker timeout", &limited), [1]);
}

#[test]
fn unknown_query_terms_count_against_coverage() {
    let index = Index::build(TITLE_FIRST, &corpus());
    let found = index.rank("queue nonexistent", &RankParams::DEFAULT);
    assert_eq!(found.len(), 1);
    assert!(
        found
            .iter()
            .all(|scored| (scored.coverage - 0.5).abs() < f64::EPSILON)
    );
    assert_eq!(
        index.rank("nonexistent words", &RankParams::DEFAULT),
        Vec::<Scored>::new()
    );
}

#[test]
fn ties_are_ordered_by_id() {
    let docs = vec![
        doc("got_b", "redis", "timeout"),
        doc("got_c", "redis", "timeout"),
        doc("got_a", "redis", "timeout"),
    ];
    let index = Index::build(TITLE_FIRST, &docs);
    let order: Vec<u32> = index
        .rank("redis", &RankParams::DEFAULT)
        .iter()
        .map(|scored| scored.doc)
        .collect();
    assert_eq!(order, [2, 0, 1]);
}

#[test]
fn an_empty_query_or_corpus_returns_nothing() {
    let index = Index::build(TITLE_FIRST, &corpus());
    assert_eq!(index.rank("", &ALL), Vec::<Scored>::new());
    assert_eq!(index.rank("the and of", &ALL), Vec::<Scored>::new());
    let empty = Index::build(TITLE_FIRST, &Vec::<Doc>::new());
    assert!(empty.is_empty());
    assert_eq!(empty.rank("worker", &ALL), Vec::<Scored>::new());
}

#[test]
fn covered_terms_count_every_field() {
    let index = Index::build(TITLE_FIRST, &corpus());
    let query = index.query_terms("worker sigterm queue redis missing");
    assert_eq!(query.distinct, 5);
    assert_eq!(query.ids.len(), 4);
    assert_eq!(index.terms_covered(0, &query.ids), 2);
    assert_eq!(index.terms_covered(1, &query.ids), 2);
    assert_eq!(index.terms_covered(2, &query.ids), 3);
}

const NOUNS: [&str; 8] = [
    "apple", "berry", "cherry", "grape", "lemon", "mango", "peach", "plum",
];
const OTHERS: [&str; 4] = ["xenon", "zebra", "quartz", "violin"];

fn field_words() -> impl Strategy<Value = Vec<&'static str>> {
    proptest::collection::vec(proptest::sample::select(&NOUNS[..]), 3)
}

proptest! {
    #[test]
    fn an_unrelated_document_never_changes_a_one_term_result(
        fields in proptest::collection::vec((field_words(), field_words()), 2..12),
        unrelated in proptest::collection::vec(proptest::sample::select(&OTHERS[..]), 6),
        term in proptest::sample::select(&NOUNS[..]),
    ) {
        const IDS: [&str; 12] = ["d00", "d01", "d02", "d03", "d04", "d05", "d06", "d07", "d08", "d09", "d10", "d11"];
        let mut docs: Vec<Doc> = fields
            .into_iter()
            .zip(IDS)
            .map(|((title, body), id)| Doc { id, fields: vec![title, body] })
            .collect();
        let before = Index::build(TITLE_FIRST, &docs).rank(term, &RankParams::DEFAULT);
        docs.push(Doc {
            id: "d99",
            fields: vec![unrelated[..3].to_vec(), unrelated[3..].to_vec()],
        });
        let after = Index::build(TITLE_FIRST, &docs).rank(term, &RankParams::DEFAULT);
        let order = |found: &[Scored]| found.iter().map(|scored| scored.doc).collect::<Vec<_>>();
        prop_assert_eq!(order(&before), order(&after));
    }
}
