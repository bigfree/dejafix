#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Analyzer;

pub(super) const MAX_STEM_BYTES: usize = 64;

#[derive(Debug, Clone, Copy)]
pub(super) struct Word {
    pub(super) bytes: [u8; MAX_STEM_BYTES],
    pub(super) len: usize,
    pub(super) p1: usize,
    pub(super) p2: usize,
}
