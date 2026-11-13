#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub old_start: &'static str,
    pub old_end: &'static str,
    pub new_start: &'static str,
    pub new_end: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrateStep {
    Rename,
    Paths(usize),
    Instructions,
    Gitignore,
    Bridges(Vec<&'static str>),
    Hooks(Vec<&'static str>),
}
