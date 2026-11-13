#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchQuery<'q> {
    pub text: &'q str,
    pub regex: bool,
    pub failed_only: bool,
}
