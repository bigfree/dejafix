pub const HOOK_MARKER_START: &str = "# >>> dejafix auto-capture >>>";
pub const HOOK_MARKER_END: &str = "# <<< dejafix auto-capture <<<";
pub const HOOK_SHEBANG: &str = "#!/bin/sh\n";
pub const OLD_HOOK_SHEBANGS: [&str; 1] = ["#!/usr/bin/env bash"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookKind {
    PostCommit,
    PostMerge,
    PreCommit,
}

impl HookKind {
    pub const ALL: [HookKind; 3] = [Self::PostCommit, Self::PostMerge, Self::PreCommit];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookChange {
    Installed(String),
    Refreshed(String),
    Current,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    Untouched,
    Delete,
    Write(String),
}
