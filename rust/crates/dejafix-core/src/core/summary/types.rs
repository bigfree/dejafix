use jiff::civil::Date;

use crate::core::event::Event;

#[derive(Debug, Clone, Copy)]
pub struct SummaryInput<'s> {
    pub project_name: &'s str,
    pub today: Date,
    pub purpose: Option<&'s str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueFile<'l> {
    pub issue: &'l str,
    pub name: Option<String>,
    pub contents: String,
}

pub(super) struct Group<'l, 'a> {
    pub(super) id: &'l str,
    pub(super) events: Vec<&'l Event<'a>>,
}

pub(super) struct Listed<'l, 'a> {
    pub(super) id: &'l str,
    pub(super) issue: &'l Event<'a>,
    pub(super) events: &'l [&'l Event<'a>],
    pub(super) fix: Option<&'l Event<'a>>,
}
