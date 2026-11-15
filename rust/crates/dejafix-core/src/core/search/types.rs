use crate::core::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Ranked,
    Exact,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchQuery<'q> {
    pub text: &'q str,
    pub mode: SearchMode,
    pub failed_only: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EventDoc<'e, 'a>(pub &'e Event<'a>);
