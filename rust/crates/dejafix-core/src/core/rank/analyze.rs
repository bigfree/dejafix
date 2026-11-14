use std::borrow::Cow;

use super::stem::stem;
use super::types::Analyzer;

pub const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with",
];

fn is_boundary(left: u8, right: u8) -> bool {
    (left.is_ascii_lowercase() && right.is_ascii_uppercase())
        || (left.is_ascii_alphabetic() && right.is_ascii_digit())
        || (left.is_ascii_digit() && right.is_ascii_alphabetic())
}

fn lower(token: &str) -> Cow<'_, str> {
    if token.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(token.to_ascii_lowercase())
    } else {
        Cow::Borrowed(token)
    }
}

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.binary_search(&word).is_ok()
}

fn push_part<'t>(part: &'t str, out: &mut Vec<Cow<'t, str>>) {
    if part.bytes().all(|byte| byte.is_ascii_digit()) {
        out.push(Cow::Borrowed(part));
        return;
    }
    if part.len() < 2 {
        return;
    }
    match lower(part) {
        Cow::Borrowed(word) => {
            if !is_stop_word(word) {
                out.push(stem(word));
            }
        }
        Cow::Owned(mut word) => {
            if is_stop_word(&word) {
                return;
            }
            let stemmed = match stem(&word) {
                Cow::Borrowed(kept) => Err(kept.len()),
                Cow::Owned(changed) => Ok(changed),
            };
            match stemmed {
                Ok(changed) => out.push(Cow::Owned(changed)),
                Err(kept) => {
                    word.truncate(kept);
                    out.push(Cow::Owned(word));
                }
            }
        }
    }
}

impl Analyzer {
    pub fn terms<'t>(self, text: &'t str, out: &mut Vec<Cow<'t, str>>) {
        for token in text
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
        {
            let mut start = 0;
            for (index, pair) in token.as_bytes().windows(2).enumerate() {
                if let [left, right] = pair
                    && is_boundary(*left, *right)
                {
                    push_part(token.get(start..=index).unwrap_or_default(), out);
                    start = index + 1;
                }
            }
            if start == 0 {
                push_part(token, out);
            } else {
                push_part(token.get(start..).unwrap_or_default(), out);
                out.push(lower(token));
            }
        }
    }
}
