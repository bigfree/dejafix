#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenConfig<'a> {
    pub client: &'a str,
    pub key: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reverted {
    pub client: String,
    pub clean_since: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PreRelease {
    DevOnly,
    Alpha(u64),
    Beta(u64),
    Candidate(u64),
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DevRelease {
    Dev(u64),
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    pub(crate) release: Vec<u64>,
    pub(crate) pre: PreRelease,
    pub(crate) post: Option<u64>,
    pub(crate) dev: DevRelease,
}
