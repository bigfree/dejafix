use super::types::{HOOK_MARKER_END, HOOK_MARKER_START, HookKind};

impl HookKind {
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::PostCommit => "post-commit",
            Self::PostMerge => "post-merge",
            Self::PreCommit => "pre-commit",
        }
    }

    #[must_use]
    pub fn snippet(self, djf: &str) -> String {
        match self {
            Self::PostCommit => auto_capture_snippet(djf, "commit"),
            Self::PostMerge => auto_capture_snippet(djf, "merge"),
            Self::PreCommit => precheck_snippet(djf),
        }
    }
}

#[must_use]
pub fn shell_path(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());
    for character in path.chars() {
        match character {
            '\\' => escaped.push('/'),
            '"' | '$' | '`' => {
                escaped.push('\\');
                escaped.push(character);
            }
            other => escaped.push(other),
        }
    }
    escaped
}

#[must_use]
pub fn auto_capture_snippet(djf_path: &str, capture_arg: &str) -> String {
    format!(
        "{HOOK_MARKER_START}\n\
         # Automatically captures development events into dejafix.\n\
         # Installed by: djf hooks install (or djf init)\n\
         # Remove with:  djf hooks uninstall\n\
         DJF_BIN=\"{djf_path}\"\n\
         if [ ! -x \"$DJF_BIN\" ]; then\n\
         \x20   DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n\
         fi\n\
         if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n\
         \x20   \"$DJF_BIN\" _auto-capture \"{capture_arg}\" >/dev/null 2>&1 &\n\
         fi\n\
         {HOOK_MARKER_END}\n"
    )
}

#[must_use]
pub fn precheck_snippet(djf_path: &str) -> String {
    format!(
        "{HOOK_MARKER_START}\n\
         # Pre-commit warning check against project memory.\n\
         # Installed by: djf hooks install (or djf init)\n\
         # Remove with:  djf hooks uninstall\n\
         # Bypass once:  git commit --no-verify\n\
         DJF_BIN=\"{djf_path}\"\n\
         if [ ! -x \"$DJF_BIN\" ]; then\n\
         \x20   DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n\
         fi\n\
         if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n\
         \x20   \"$DJF_BIN\" precheck --level warn || true\n\
         fi\n\
         {HOOK_MARKER_END}\n"
    )
}
