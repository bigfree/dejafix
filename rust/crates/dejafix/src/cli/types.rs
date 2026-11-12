use std::path::Path;

use dejafix_core::{ClientConfig, WiringError};

#[derive(Debug)]
pub enum Outcome {
    Done,
    Failed(WiringError),
    Exit(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Green,
    Red,
    Yellow,
    Cyan,
    Bold,
    GreenBold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy)]
pub struct McpBlock<'a> {
    pub command: &'a str,
    pub root: &'a Path,
    pub single: bool,
    pub others: usize,
    pub pinned: &'a [ClientConfig],
    pub platform: Platform,
    pub color: bool,
}
