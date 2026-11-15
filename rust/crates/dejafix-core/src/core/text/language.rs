use std::cmp::Ordering;

use super::types::{LanguageReason, LanguageTally, LanguageVerdict};

const MAX_NON_ASCII_PERCENT: usize = 2;
const MIN_WORDS_TO_JUDGE: usize = 8;
const MIN_FUNCTION_WORD_PERCENT: usize = 8;

pub(super) const FUNCTION_WORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "be", "because",
    "been", "before", "but", "by", "can", "could", "did", "do", "does", "for", "from", "had",
    "has", "have", "how", "if", "in", "into", "is", "it", "its", "just", "may", "more", "most",
    "no", "not", "of", "on", "only", "or", "other", "our", "out", "over", "so", "some", "such",
    "than", "that", "the", "their", "then", "there", "these", "they", "this", "those", "to",
    "under", "up", "use", "used", "was", "we", "were", "what", "when", "where", "which", "while",
    "who", "why", "will", "with", "would", "you", "your",
];

fn is_latin_letter(c: char) -> bool {
    matches!(
        c,
        'a'..='z'
            | 'A'..='Z'
            | '\u{aa}'
            | '\u{ba}'
            | '\u{c0}'..='\u{2af}'
            | '\u{1e00}'..='\u{1eff}'
            | '\u{2c60}'..='\u{2c7f}'
            | '\u{a720}'..='\u{a7ff}'
            | '\u{ab30}'..='\u{ab6f}'
            | '\u{ff21}'..='\u{ff3a}'
            | '\u{ff41}'..='\u{ff5a}'
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

fn compare_folded(listed: &str, word: &str) -> Ordering {
    listed
        .bytes()
        .cmp(word.bytes().map(|byte| byte.to_ascii_lowercase()))
}

impl LanguageTally {
    fn word(&mut self, word: Option<&str>) {
        self.words += 1;
        if let Some(word) = word
            && FUNCTION_WORDS
                .binary_search_by(|listed| compare_folded(listed, word))
                .is_ok()
        {
            self.function_words += 1;
        }
    }

    fn verdict(&self) -> LanguageVerdict {
        if self.non_ascii * 100 > self.letters * MAX_NON_ASCII_PERCENT {
            return LanguageVerdict::Rejected {
                reason: LanguageReason::TooManyNonAscii,
            };
        }
        if self.words >= MIN_WORDS_TO_JUDGE
            && self.function_words * 100 < self.words * MIN_FUNCTION_WORD_PERCENT
        {
            return LanguageVerdict::Doubtful;
        }
        LanguageVerdict::English
    }
}

#[must_use]
pub fn language_verdict(text: &str) -> LanguageVerdict {
    let mut tally = LanguageTally::default();
    let mut start: Option<usize> = None;
    for (offset, c) in text.char_indices() {
        if c.is_alphabetic() {
            if !c.is_ascii() {
                if !is_latin_letter(c) {
                    return LanguageVerdict::Rejected {
                        reason: LanguageReason::NonLatinScript,
                    };
                }
                tally.non_ascii += 1;
            }
            tally.letters += 1;
        }
        if is_word_char(c) {
            start.get_or_insert(offset);
        } else if let Some(begin) = start.take() {
            tally.word(text.get(begin..offset));
        }
    }
    if let Some(begin) = start {
        tally.word(text.get(begin..));
    }
    tally.verdict()
}
