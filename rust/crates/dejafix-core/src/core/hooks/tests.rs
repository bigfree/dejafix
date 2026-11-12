use super::{
    HOOK_MARKER_END, HOOK_MARKER_START, HOOK_SHEBANG, HookChange, HookKind, Removal,
    auto_capture_snippet, precheck_snippet, remove_blocks, shell_path, sync_hook,
};

const WINDOWS_DJF: &str = r"C:\Users\ripon\AppData\Local\Programs\Python\Python312\Scripts\djf.exe";
const WINDOWS_DJF_SHELL: &str =
    "C:/Users/ripon/AppData/Local/Programs/Python/Python312/Scripts/djf.exe";

const OLD_SHEBANG: &str = "#!/usr/bin/env bash\n";
const OLD_POST_COMMIT: &str = concat!(
    "# >>> dejafix auto-capture >>>\n",
    "# Automatically captures development events into dejafix.\n",
    "# Installed by: djf hooks install (or djf init)\n",
    "# Remove with:  djf hooks uninstall\n",
    "DJF_BIN=\"/opt/anaconda3/bin/djf\"\n",
    "if [ ! -x \"$DJF_BIN\" ]; then\n",
    "    DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n",
    "fi\n",
    "if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n",
    "    \"$DJF_BIN\" _auto-capture \"commit\" >/dev/null 2>&1 &\n",
    "fi\n",
    "# <<< dejafix auto-capture <<<\n",
);
const OLD_PRE_COMMIT: &str = concat!(
    "# >>> dejafix auto-capture >>>\n",
    "# Pre-commit warning check against project memory.\n",
    "# Installed by: djf hooks install (or djf init)\n",
    "# Remove with:  djf hooks uninstall\n",
    "# Bypass once:  git commit --no-verify\n",
    "DJF_BIN=\"/opt/anaconda3/bin/djf\"\n",
    "if [ ! -x \"$DJF_BIN\" ]; then\n",
    "    DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n",
    "fi\n",
    "if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n",
    "    \"$DJF_BIN\" precheck --level warn || true\n",
    "fi\n",
    "# <<< dejafix auto-capture <<<\n",
);

const POST_COMMIT_SNIPPET: &str = concat!(
    "# >>> dejafix auto-capture >>>\n",
    "# Automatically captures development events into dejafix.\n",
    "# Installed by: djf hooks install (or djf init)\n",
    "# Remove with:  djf hooks uninstall\n",
    "DJF_BIN=\"/opt/example/bin/djf\"\n",
    "if [ ! -x \"$DJF_BIN\" ]; then\n",
    "    DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n",
    "fi\n",
    "if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n",
    "    \"$DJF_BIN\" _auto-capture \"commit\" >/dev/null 2>&1 &\n",
    "fi\n",
    "# <<< dejafix auto-capture <<<\n",
);
const PRE_COMMIT_SNIPPET: &str = concat!(
    "# >>> dejafix auto-capture >>>\n",
    "# Pre-commit warning check against project memory.\n",
    "# Installed by: djf hooks install (or djf init)\n",
    "# Remove with:  djf hooks uninstall\n",
    "# Bypass once:  git commit --no-verify\n",
    "DJF_BIN=\"/opt/example/bin/djf\"\n",
    "if [ ! -x \"$DJF_BIN\" ]; then\n",
    "    DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n",
    "fi\n",
    "if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n",
    "    \"$DJF_BIN\" precheck --level warn || true\n",
    "fi\n",
    "# <<< dejafix auto-capture <<<\n",
);

fn refreshed(change: HookChange) -> String {
    match change {
        HookChange::Refreshed(text) | HookChange::Installed(text) => text,
        other => panic!("expected new text, got {other:?}"),
    }
}

#[test]
fn the_shebang_is_posix_sh() {
    assert_eq!(HOOK_SHEBANG, "#!/bin/sh\n");
}

#[test]
fn hook_kinds_list_their_files_in_install_order() {
    let names: Vec<&str> = HookKind::ALL.iter().map(|kind| kind.file_name()).collect();
    assert_eq!(names, ["post-commit", "post-merge", "pre-commit"]);
}

#[test]
fn hook_kinds_pick_their_snippet() {
    let djf = "/opt/example/bin/djf";
    assert_eq!(HookKind::PostCommit.snippet(djf), POST_COMMIT_SNIPPET);
    assert_eq!(
        HookKind::PostMerge.snippet(djf),
        POST_COMMIT_SNIPPET.replace("\"commit\"", "\"merge\"")
    );
    assert_eq!(HookKind::PreCommit.snippet(djf), PRE_COMMIT_SNIPPET);
}

#[test]
fn the_precheck_snippet_bakes_the_path_and_keeps_the_markers() {
    let snippet = precheck_snippet("/opt/example/bin/djf");
    assert!(snippet.contains("DJF_BIN=\"/opt/example/bin/djf\""));
    assert!(snippet.contains(HOOK_MARKER_START));
    assert!(snippet.contains(HOOK_MARKER_END));
    assert!(snippet.contains("command -v djf"));
    assert_eq!(snippet, PRE_COMMIT_SNIPPET);
}

#[test]
fn the_auto_capture_snippet_bakes_the_path_and_the_argument() {
    let snippet = auto_capture_snippet("/opt/example/bin/djf", "commit");
    assert!(snippet.contains("DJF_BIN=\"/opt/example/bin/djf\""));
    assert!(snippet.contains("\"$DJF_BIN\" _auto-capture \"commit\""));
    assert!(!snippet.contains("$1"));
    assert!(snippet.contains(">/dev/null 2>&1 &"));
    assert!(!snippet.contains("2>/dev/null &"));
    assert!(snippet.starts_with(HOOK_MARKER_START));
    assert!(snippet.ends_with(&format!("{HOOK_MARKER_END}\n")));
}

#[test]
fn shell_path_turns_backslashes_into_slashes() {
    assert_eq!(shell_path(WINDOWS_DJF), WINDOWS_DJF_SHELL);
    assert!(!shell_path(WINDOWS_DJF).contains('\\'));
    assert_eq!(shell_path("/usr/local/bin/djf"), "/usr/local/bin/djf");
}

#[test]
fn shell_path_escapes_what_a_double_quoted_string_interprets() {
    assert_eq!(shell_path("/a\"b"), "/a\\\"b");
    assert_eq!(shell_path("/a$b"), "/a\\$b");
    assert_eq!(shell_path("/a`b"), "/a\\`b");
    assert_eq!(shell_path("C:\\a\"b"), "C:/a\\\"b");
}

#[cfg(unix)]
#[test]
fn an_escaped_path_reaches_the_shell_unchanged() {
    let path = "/tmp/a\"b$(echo pwned)`echo pwned`\\c $HOME";
    let script = format!("DJF_BIN=\"{}\"\nprintf %s \"$DJF_BIN\"", shell_path(path));
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "/tmp/a\"b$(echo pwned)`echo pwned`/c $HOME"
    );
}

#[test]
fn a_missing_file_gets_a_shebang_and_the_snippet() {
    assert_eq!(
        sync_hook(None, POST_COMMIT_SNIPPET),
        HookChange::Installed(format!("{HOOK_SHEBANG}{POST_COMMIT_SNIPPET}"))
    );
}

#[test]
fn a_file_without_our_block_gets_the_snippet_after_a_blank_line() {
    let existing = "#!/bin/sh\necho mine\n\n\n";
    assert_eq!(
        sync_hook(Some(existing), PRE_COMMIT_SNIPPET),
        HookChange::Installed(format!("#!/bin/sh\necho mine\n\n{PRE_COMMIT_SNIPPET}"))
    );
}

#[test]
fn a_hook_of_an_old_release_gets_the_new_path_and_shebang() {
    let existing = format!("{OLD_SHEBANG}{OLD_POST_COMMIT}");
    let text = refreshed(sync_hook(Some(&existing), POST_COMMIT_SNIPPET));
    assert_eq!(text, format!("{HOOK_SHEBANG}{POST_COMMIT_SNIPPET}"));
    assert!(!text.contains("anaconda"));
    assert_eq!(text.matches(HOOK_MARKER_START).count(), 1);

    let existing = format!("{OLD_SHEBANG}{OLD_PRE_COMMIT}");
    let text = refreshed(sync_hook(Some(&existing), PRE_COMMIT_SNIPPET));
    assert_eq!(text, format!("{HOOK_SHEBANG}{PRE_COMMIT_SNIPPET}"));
}

#[test]
fn a_users_own_lines_and_shebang_survive_a_refresh_byte_for_byte() {
    let before = "#!/usr/bin/env bash\nset -e\necho 'lint first'\n\n";
    let after = "\n# then tests\npytest -q  \n\ttrailing\n";
    let existing = format!("{before}{OLD_PRE_COMMIT}{after}");
    let text = refreshed(sync_hook(Some(&existing), PRE_COMMIT_SNIPPET));
    assert!(text.starts_with(before));
    assert!(text.ends_with(after));
    assert_eq!(text, format!("{before}{PRE_COMMIT_SNIPPET}{after}"));
}

#[test]
fn an_old_shebang_stays_when_something_else_is_outside_the_markers() {
    let existing = format!("{OLD_SHEBANG}{OLD_POST_COMMIT}echo after\n");
    let text = refreshed(sync_hook(Some(&existing), POST_COMMIT_SNIPPET));
    assert!(text.starts_with(OLD_SHEBANG));
    assert!(text.ends_with("echo after\n"));
}

#[test]
fn an_old_shebang_goes_when_only_blocks_and_whitespace_are_outside() {
    let existing = format!("{OLD_SHEBANG}\n{OLD_POST_COMMIT}\n  \n{OLD_PRE_COMMIT}");
    let text = refreshed(sync_hook(Some(&existing), POST_COMMIT_SNIPPET));
    assert!(text.starts_with(HOOK_SHEBANG));
}

#[test]
fn a_second_sync_changes_nothing() {
    let existing = format!("{OLD_SHEBANG}{OLD_POST_COMMIT}");
    let first = refreshed(sync_hook(Some(&existing), POST_COMMIT_SNIPPET));
    assert_eq!(
        sync_hook(Some(&first), POST_COMMIT_SNIPPET),
        HookChange::Current
    );

    let mine = format!("#!/bin/sh\necho mine\n{OLD_PRE_COMMIT}");
    let first = refreshed(sync_hook(Some(&mine), PRE_COMMIT_SNIPPET));
    assert_eq!(
        sync_hook(Some(&first), PRE_COMMIT_SNIPPET),
        HookChange::Current
    );
}

#[test]
fn a_block_without_an_end_marker_is_skipped() {
    let broken = format!(
        "{OLD_SHEBANG}echo mine\n{}echo also mine\n",
        OLD_PRE_COMMIT.replace("# <<< dejafix auto-capture <<<\n", "")
    );
    assert_eq!(
        sync_hook(Some(&broken), PRE_COMMIT_SNIPPET),
        HookChange::Skipped
    );
}

#[test]
fn a_block_without_a_final_newline_is_replaced() {
    let existing = OLD_PRE_COMMIT.trim_end_matches('\n');
    let text = refreshed(sync_hook(Some(existing), PRE_COMMIT_SNIPPET));
    assert_eq!(text, PRE_COMMIT_SNIPPET);
}

#[test]
fn uninstall_leaves_a_file_without_our_block_alone() {
    assert_eq!(remove_blocks("#!/bin/sh\necho mine\n"), Removal::Untouched);
    assert_eq!(remove_blocks(""), Removal::Untouched);
}

#[test]
fn uninstall_deletes_a_file_that_is_only_ours() {
    let ours = format!("{HOOK_SHEBANG}{POST_COMMIT_SNIPPET}");
    assert_eq!(remove_blocks(&ours), Removal::Delete);
    let old = format!("{OLD_SHEBANG}{OLD_PRE_COMMIT}");
    assert_eq!(remove_blocks(&old), Removal::Delete);
    assert_eq!(remove_blocks(POST_COMMIT_SNIPPET), Removal::Delete);
}

#[test]
fn uninstall_keeps_the_users_lines_with_one_trailing_newline() {
    let existing = format!("#!/bin/sh\necho mine\n\n{POST_COMMIT_SNIPPET}\n\n");
    assert_eq!(
        remove_blocks(&existing),
        Removal::Write("#!/bin/sh\necho mine\n".to_owned())
    );
    let between = format!("echo a\n{POST_COMMIT_SNIPPET}echo b\n");
    assert_eq!(
        remove_blocks(&between),
        Removal::Write("echo a\necho b\n".to_owned())
    );
}

#[test]
fn uninstall_drops_everything_from_a_start_marker_without_an_end() {
    let existing = format!("echo mine\n{HOOK_MARKER_START}\nstuff\n");
    assert_eq!(
        remove_blocks(&existing),
        Removal::Write("echo mine\n".to_owned())
    );
}

#[test]
fn uninstall_removes_two_blocks() {
    let existing = format!("echo a\n{POST_COMMIT_SNIPPET}echo b\n{PRE_COMMIT_SNIPPET}echo c\n");
    assert_eq!(
        remove_blocks(&existing),
        Removal::Write("echo a\necho b\necho c\n".to_owned())
    );
}
