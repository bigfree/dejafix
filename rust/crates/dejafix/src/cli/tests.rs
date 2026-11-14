use std::path::{Path, PathBuf};

use dejafix_core::{ClientConfig, ConfigState};

use super::init::write_mcp_block;
use super::output::{json_escaped, secho};
use super::types::{McpBlock, Platform, Style};

fn block(platform: Platform, single: bool, pinned: &[ClientConfig], color: bool) -> String {
    let mut out = Vec::new();
    write_mcp_block(
        &mut out,
        &McpBlock {
            command: "/opt/djf/bin/djf-mcp",
            root: Path::new("/work/myproj"),
            single,
            others: 2,
            pinned,
            platform,
            color,
        },
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn the_shared_block_is_stable() {
    insta::assert_snapshot!(block(Platform::Mac, false, &[], false));
}

#[test]
fn the_single_project_block_is_stable() {
    insta::assert_snapshot!(block(Platform::Linux, true, &[], false));
}

#[test]
fn printed_locations_match_the_platform() {
    let linux = block(Platform::Linux, false, &[], false);
    assert!(linux.contains("~/.config/Claude/claude_desktop_config.json"));
    assert!(!linux.contains("Library/Application Support"));
    let mac = block(Platform::Mac, false, &[], false);
    assert!(mac.contains("~/Library/Application Support/Claude/claude_desktop_config.json"));
}

#[test]
fn windows_locations_use_windows_conventions() {
    let windows = block(Platform::Windows, false, &[], false);
    assert!(windows.contains(r"%USERPROFILE%\.cursor\mcp.json"));
    assert!(windows.contains(r"%USERPROFILE%\.codex\config.toml"));
    assert!(windows.contains(r"%APPDATA%\Claude\claude_desktop_config.json"));
    assert!(!windows.contains("~/.cursor"));
}

#[test]
fn a_pinned_config_is_named_in_yellow_only_on_a_terminal() {
    let pinned = [ClientConfig {
        client: "Cursor",
        path: PathBuf::from("/home/u/.cursor/mcp.json"),
        state: ConfigState::Pinned,
    }];
    let plain = block(Platform::Mac, false, &pinned, false);
    assert!(plain.contains("  ⚠ An existing config still pins dejafix to one repo:\n      Cursor  /home/u/.cursor/mcp.json\n"));
    let coloured = block(Platform::Mac, false, &pinned, true);
    assert!(
        coloured
            .contains("\x1b[33m  ⚠ An existing config still pins dejafix to one repo:\x1b[0m\n")
    );
    assert!(!block(Platform::Mac, true, &pinned, false).contains("still pins"));
}

#[test]
fn json_escaping_follows_python_ensure_ascii() {
    assert_eq!(
        json_escaped("C:\\Users\\Žofie\\\"x\"\n\t\u{1}😀"),
        "C:\\\\Users\\\\\\u017dofie\\\\\\\"x\\\"\\n\\t\\u0001\\ud83d\\ude00"
    );
}

#[test]
fn secho_colours_only_when_asked() {
    let mut out = Vec::new();
    secho(&mut out, "ok", Style::Green, true).unwrap();
    secho(&mut out, "bad", Style::Red, true).unwrap();
    secho(&mut out, "info", Style::Cyan, false).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "\x1b[32mok\x1b[0m\n\x1b[31mbad\x1b[0m\ninfo\n"
    );
}

#[test]
fn a_root_with_quotes_stays_valid_json_and_toml() {
    let mut out = Vec::new();
    write_mcp_block(
        &mut out,
        &McpBlock {
            command: "djf-mcp",
            root: Path::new("/work/x\"],\"command\":\"sh"),
            single: true,
            others: 0,
            pinned: &[],
            platform: Platform::Linux,
            color: false,
        },
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains(r#""args": ["--root", "/work/x\"],\"command\":\"sh"]"#));
    assert!(text.contains(r#"args = ["--root", "/work/x\"],\"command\":\"sh"]"#));
}

#[test]
fn printable_escapes_c0_del_and_c1_only() {
    use std::path::Path;
    assert_eq!(super::output::printable(Path::new("/a/b c/ü")), "/a/b c/ü");
    assert_eq!(
        super::output::printable(Path::new("/a/x\u{1b}]52;c;aGk=\u{7}y")),
        "/a/x\\x1b]52;c;aGk=\\x07y"
    );
    assert_eq!(super::output::printable(Path::new("/a/\u{7f}")), "/a/\\x7f");
    assert_eq!(
        super::output::printable(Path::new("/a/\u{9b}31m")),
        "/a/\\u{9b}31m"
    );
}

#[test]
fn printable_text_escapes_error_messages_too() {
    assert_eq!(
        super::output::printable_text("cannot write /a/x\u{1b}]52;c;aGk=\u{7}y"),
        "cannot write /a/x\\x1b]52;c;aGk=\\x07y"
    );
}
