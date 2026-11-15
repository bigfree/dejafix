pub struct PythonLines<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) start: usize,
    pub(super) number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecimalKey {
    pub(super) length: usize,
    pub(super) digits: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LanguageReason {
    #[error("it contains letters outside the Latin script")]
    NonLatinScript,
    #[error("too many letters outside ASCII")]
    TooManyNonAscii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageVerdict {
    English,
    Doubtful,
    Rejected { reason: LanguageReason },
}

#[derive(Debug, Default)]
pub(super) struct LanguageTally {
    pub(super) letters: usize,
    pub(super) non_ascii: usize,
    pub(super) words: usize,
    pub(super) function_words: usize,
}
