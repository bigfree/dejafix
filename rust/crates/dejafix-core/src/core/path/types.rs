use std::borrow::Cow;

pub trait PathResolver {
    fn resolve(&self, path: &str) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathContext {
    pub root: String,
    pub cwd: String,
    pub fold_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelativePath<'a>(pub(super) Cow<'a, str>);

pub(super) enum Never {}
