use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record<'a> {
    pub kind: Cow<'a, str>,
    pub id: Cow<'a, str>,
    pub issue_id: Cow<'a, str>,
    pub outcome: Cow<'a, str>,
    pub summary: Cow<'a, str>,
    pub location: Cow<'a, str>,
    pub timestamp: Cow<'a, str>,
    pub capture_source: Cow<'a, str>,
    pub supersedes: Option<Cow<'a, str>>,
    pub files: Vec<Cow<'a, str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonKind {
    Issue,
    Attempt,
    Decision,
}

#[derive(Debug)]
pub struct Candidate<'r, 'a> {
    pub(super) kind: LessonKind,
    pub(super) record: &'r Record<'a>,
    pub(super) files: HashSet<String>,
    pub(super) mention_text: &'r str,
}

#[derive(Debug)]
pub struct Memory<'r, 'a> {
    pub(super) candidates: Vec<Candidate<'r, 'a>>,
}

#[derive(Debug, Default)]
pub(super) struct Found<'r, 'a> {
    pub(super) issues: Vec<&'r Record<'a>>,
    pub(super) attempts: Vec<&'r Record<'a>>,
    pub(super) decisions: Vec<&'r Record<'a>>,
}

pub(super) struct Normalizer<'c, 's, R> {
    pub(super) root: &'c str,
    pub(super) prefix: String,
    pub(super) resolver: &'c R,
    pub(super) cache: HashMap<&'s str, String>,
}
