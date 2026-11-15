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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldSpec {
    pub name: &'static str,
    pub weight: f64,
}

pub trait Document {
    fn id(&self) -> &str;
    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str));
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankParams {
    pub k1: f64,
    pub b: f64,
    pub min_coverage: f64,
    pub min_ratio: f64,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub doc: u32,
    pub score: f64,
    pub coverage: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryTerms {
    pub ids: Vec<u32>,
    pub distinct: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    pub(super) fields: &'static [FieldSpec],
    pub(super) terms: Vec<Box<str>>,
    pub(super) document_frequency: Vec<u32>,
    pub(super) offsets: Vec<u32>,
    pub(super) postings: Vec<(u32, u16)>,
    pub(super) lengths: Vec<u16>,
    pub(super) average: Vec<f64>,
    pub(super) order: Vec<u32>,
}
