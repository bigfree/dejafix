use crate::core::global::Entry;
use crate::core::log::EventLog;
use crate::core::stale::Stale;

#[derive(Debug, Clone, Copy)]
pub struct BriefInput<'i, 'a> {
    pub name: &'i str,
    pub log: &'i EventLog<'a>,
    pub stale: &'i [Stale<'i, 'a>],
    pub gotchas: &'i [&'i Entry],
    pub now: jiff::Timestamp,
    pub color: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Codes {
    pub(super) bold: &'static str,
    pub(super) dim: &'static str,
    pub(super) yellow: &'static str,
    pub(super) red: &'static str,
    pub(super) teal: &'static str,
    pub(super) green: &'static str,
    pub(super) reset: &'static str,
}

impl Codes {
    pub(super) const COLOR: Self = Self {
        bold: "\x1b[1m",
        dim: "\x1b[2m",
        yellow: "\x1b[33m",
        red: "\x1b[31m",
        teal: "\x1b[36m",
        green: "\x1b[32m",
        reset: "\x1b[0m",
    };

    pub(super) const PLAIN: Self = Self {
        bold: "",
        dim: "",
        yellow: "",
        red: "",
        teal: "",
        green: "",
        reset: "",
    };
}
