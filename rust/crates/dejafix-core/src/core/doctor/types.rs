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
