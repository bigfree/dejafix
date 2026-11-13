#[derive(Debug, Clone, Copy)]
pub(super) enum Parser {
    Pyproject,
    Requirements,
    PackageJson,
    Cargo,
    GoMod,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Detector {
    pub(super) file: &'static str,
    pub(super) tags: &'static [&'static str],
    pub(super) parser: Option<Parser>,
    pub(super) directory: bool,
}

#[derive(Debug, Default)]
pub(super) struct Found {
    pub(super) tags: Vec<String>,
    pub(super) libraries: Vec<String>,
}
