use super::{
    BRIDGE_BLOCK, HOOK_BLOCK, MigrateStep, convert_block, has_legacy_path, is_stale_config,
    rewrite_legacy_paths,
};

const OLD_HOOK: &str = "#!/bin/sh\n\
echo user hook\n\
# >>> projectmem auto-capture >>>\n\
pjm auto-capture\n\
# <<< projectmem auto-capture <<<\n";

#[test]
fn the_markers_are_renamed_when_no_dejafix_block_exists() {
    let converted = convert_block(OLD_HOOK, &HOOK_BLOCK).unwrap();
    assert_eq!(
        converted,
        "#!/bin/sh\necho user hook\n# >>> dejafix auto-capture >>>\npjm auto-capture\n# <<< dejafix auto-capture <<<\n"
    );
}

#[test]
fn every_old_block_is_dropped_when_a_dejafix_block_exists() {
    let text = format!(
        "own\n{}\nold one\n{}\nmiddle\n{}\nold two\n{}tail\n<!-- >>> dejafix bridge >>> -->\nnew\n<!-- <<< dejafix bridge <<< -->\n",
        BRIDGE_BLOCK.old_start, BRIDGE_BLOCK.old_end, BRIDGE_BLOCK.old_start, BRIDGE_BLOCK.old_end
    );
    let converted = convert_block(&text, &BRIDGE_BLOCK).unwrap();
    assert_eq!(
        converted,
        "own\nmiddle\ntail\n<!-- >>> dejafix bridge >>> -->\nnew\n<!-- <<< dejafix bridge <<< -->\n"
    );
}

#[test]
fn an_unterminated_old_block_is_kept_beside_a_dejafix_block() {
    let text = format!(
        "{}\nold\n{}\nnew\n{}\n",
        HOOK_BLOCK.old_start, HOOK_BLOCK.new_start, HOOK_BLOCK.new_end
    );
    assert_eq!(convert_block(&text, &HOOK_BLOCK).unwrap(), text);
}

#[test]
fn a_file_without_the_old_block_is_not_converted() {
    assert_eq!(convert_block("plain\n", &HOOK_BLOCK), None);
}

#[test]
fn only_whole_legacy_folder_names_are_rewritten() {
    assert_eq!(
        rewrite_legacy_paths("see .projectmem/summary.md and x.projectmem-y, not .projectmems")
            .unwrap(),
        "see .dejafix/summary.md and x.dejafix-y, not .projectmems"
    );
    assert_eq!(rewrite_legacy_paths("nothing here .projectmems"), None);
    assert!(has_legacy_path(".projectmem\n"));
    assert!(!has_legacy_path(".projectmem_x é.projectmemé"));
}

#[test]
fn every_legacy_config_token_marks_a_stale_config() {
    for token in [
        "projectmem.mcp_server",
        "mcp_servers.projectmem",
        "\"projectmem\":",
        "pjm-mcp",
        "pjm-edit-hook",
        "PJM_EDIT_HOOK_LOG",
        "PROJECTMEM_ROOT",
    ] {
        assert!(is_stale_config(&format!("x {token} y")), "{token}");
    }
}

#[test]
fn a_folder_named_projectmem_is_not_a_stale_config() {
    assert!(!is_stale_config(
        "[projects.\"/src/projectmem\"]\ntrust_level = \"trusted\"\n"
    ));
}

#[test]
fn each_step_reads_as_python_prints_it() {
    let lines: Vec<String> = [
        MigrateStep::Rename,
        MigrateStep::Paths(2),
        MigrateStep::Instructions,
        MigrateStep::Gitignore,
        MigrateStep::Bridges(vec!["CLAUDE.md", "AGENTS.md"]),
        MigrateStep::Hooks(vec!["post-commit", "pre-commit"]),
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        lines,
        [
            "rename .projectmem/ -> .dejafix/",
            "update .projectmem paths recorded in 2 memory files",
            "refresh AI_INSTRUCTIONS.md (previous copy kept as .bak)",
            "update .gitignore entries",
            "replace the bridge block in CLAUDE.md, AGENTS.md",
            "reinstall git hooks: post-commit, pre-commit",
        ]
    );
}
