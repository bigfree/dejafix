use super::{
    BRIDGE_END, BRIDGE_START, BridgeChange, INSTRUCTIONS_VERSION, ManifestInfo, TITLE_CHARS,
    ai_instructions, apply_bridge, bridge_block, extract_global_block, gitignore_with,
    initial_plan, initial_project_map, initial_summary, instructions_version, plan_with_idea,
    project_map_from_stack, splice_global_block, stack_summary_suffix,
};
use crate::core::global::Stack;
use crate::core::summary::RECORD_CHARS;

const STALE: &str = "# dejafix AI Instructions\n\nCall `precheck_file(path)` before editing.\n\n## Global Memory — Inherited Knowledge\n\n### Library Gotchas\n\n- **vite**: dev server caches deps (from other-app)\n\n## Rules summary\n\n- old rule\n";

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn the_template_carries_the_current_marker() {
    assert_eq!(
        instructions_version(ai_instructions()),
        INSTRUCTIONS_VERSION
    );
    assert_eq!(instructions_version(STALE), 0);
    assert_eq!(
        instructions_version("<!--  dejafix-instructions:\u{3000}١٢ -->"),
        12
    );
    assert_eq!(
        instructions_version("<!-- dejafix-instructions: 99999999999 -->"),
        u32::MAX
    );
    assert_eq!(instructions_version("<!-- dejafix-instructions: -->"), 0);
}

#[test]
fn the_instructions_state_record_lengths_and_supersedes() {
    let text = ai_instructions();
    assert!(text.contains(&format!("issue title under {TITLE_CHARS} characters")));
    assert!(text.contains(&format!(
        "clips longer records at {RECORD_CHARS} characters"
    )));
    let lowered = text.to_lowercase();
    assert!(lowered.contains("supersede"));
    assert!(lowered.contains("append-only"));
    assert!(lowered.contains("summary.md"));
    assert!(text.contains("supersedes=") || text.contains("--supersedes"));
    assert!(text.contains("precheck_file(file_path)"));
    insta::assert_snapshot!(text);
}

#[test]
fn a_global_block_is_extracted_up_to_the_next_heading() {
    assert_eq!(
        extract_global_block(STALE).as_deref(),
        Some(
            "## Global Memory — Inherited Knowledge\n\n### Library Gotchas\n\n- **vite**: dev server caches deps (from other-app)\n"
        )
    );
    assert_eq!(extract_global_block("no block here"), None);
    assert_eq!(
        extract_global_block("x\n## Global Memory — Inherited Knowledge\nbody\n\n\n").as_deref(),
        Some("## Global Memory — Inherited Knowledge\nbody\n")
    );
}

#[test]
fn a_spliced_block_replaces_the_old_one_before_the_rules() {
    let block = extract_global_block(STALE).unwrap();
    let fresh = splice_global_block(ai_instructions(), &block);
    assert_eq!(
        fresh
            .matches("## Global Memory — Inherited Knowledge")
            .count(),
        1
    );
    assert!(fresh.find("Global Memory").unwrap() < fresh.find("## Rules summary").unwrap());
    let again = splice_global_block(&fresh, "## Global Memory — Inherited Knowledge\nnew\n");
    assert_eq!(again.matches("## Global Memory").count(), 1);
    assert!(again.contains("Inherited Knowledge\nnew\n\n## Rules summary"));
    assert!(!again.contains("vite"));
}

#[test]
fn a_block_goes_to_the_end_without_a_rules_heading() {
    assert_eq!(
        splice_global_block(
            "# Title\n\n\n",
            "## Global Memory — Inherited Knowledge\nx\n"
        ),
        "# Title\n\n## Global Memory — Inherited Knowledge\nx\n"
    );
    assert_eq!(
        splice_global_block(
            "a\n## Global Memory — Inherited Knowledge\nold\n\n## Next\nb\n",
            "NEW\n"
        ),
        "a\n\n## Next\nb\n\nNEW\n"
    );
}

#[test]
fn the_bridge_names_the_project() {
    let bridge = bridge_block(Some("checkout-api"));
    assert!(bridge.contains("checkout-api"));
    assert!(bridge.contains("project=\"checkout-api\""));
    assert!(bridge.starts_with(BRIDGE_START));
    assert!(bridge.ends_with(&format!("{BRIDGE_END}\n")));
    assert_eq!(bridge.matches(BRIDGE_END).count(), 1);
    insta::assert_snapshot!(bridge);
}

#[test]
fn the_bridge_is_still_valid_without_a_name() {
    for name in [None, Some("")] {
        let bridge = bridge_block(name);
        assert!(bridge.contains("dejafix (MANDATORY)"));
        assert!(!bridge.contains("registered with dejafix as"));
        assert!(bridge.to_lowercase().contains("supersede"));
    }
}

#[test]
fn a_bridge_is_created_appended_refreshed_or_left_alone() {
    let BridgeChange::Created(created) = apply_bridge(None, "CLAUDE.md", Some("app")) else {
        panic!("not created");
    };
    assert_eq!(
        created,
        format!("# CLAUDE.md\n\n{}", bridge_block(Some("app")))
    );
    assert_eq!(
        apply_bridge(Some(&created), "CLAUDE.md", Some("app")),
        BridgeChange::Unchanged
    );
    let BridgeChange::Refreshed(renamed) = apply_bridge(Some(&created), "CLAUDE.md", Some("web"))
    else {
        panic!("not refreshed");
    };
    assert_eq!(renamed.matches(BRIDGE_START).count(), 1);
    assert!(renamed.contains("**web**"));
    let BridgeChange::Appended(appended) = apply_bridge(
        Some("# House rules\n\nAlways run the tests.\n\n\n"),
        "AGENTS.md",
        None,
    ) else {
        panic!("not appended");
    };
    assert_eq!(
        appended,
        format!(
            "# House rules\n\nAlways run the tests.\n\n{}",
            bridge_block(None)
        )
    );
}

#[test]
fn a_reversed_marker_pair_is_spliced_as_python_slices_it() {
    let content = format!("a{BRIDGE_END}b{BRIDGE_START}c");
    let BridgeChange::Refreshed(text) = apply_bridge(Some(&content), "CLAUDE.md", None) else {
        panic!("not refreshed");
    };
    let bridge = bridge_block(None);
    assert_eq!(
        text,
        format!("a{BRIDGE_END}b{}b{BRIDGE_START}c", bridge.trim_end())
    );
}

#[test]
fn initial_files_name_the_project() {
    let summary = initial_summary("demo");
    assert!(summary.starts_with("# dejafix - demo\n"));
    assert!(summary.contains("Project purpose") && summary.contains("AI assistants"));
    let map = initial_project_map("demo");
    assert!(map.contains("Status: not created yet"));
    assert!(map.contains("## Structure") && map.contains("## Relationships"));
    let plan = initial_plan("demo");
    assert!(plan.starts_with("# demo — plan\n"));
    for section in ["## Ideas", "## Active plans", "## Next", "## Shipped"] {
        assert!(plan.contains(section), "{section}");
    }
    assert!(plan.contains("NOT the event log"));
}

#[test]
fn gitignore_entries_are_added_once() {
    let first = gitignore_with("", &[]).unwrap();
    assert_eq!(
        first,
        ".dejafix/events.jsonl\n.dejafix/watch.pid\n.dejafix/watch.log\n.dejafix/structure.json\n.djf-*\n"
    );
    assert_eq!(
        gitignore_with(
            ".dejafix/events.jsonl\n.dejafix/watch.pid\n.dejafix/watch.log\n.dejafix/structure.json\n",
            &[]
        )
        .unwrap(),
        ".dejafix/events.jsonl\n.dejafix/watch.pid\n.dejafix/watch.log\n.dejafix/structure.json\n\n.djf-*\n"
    );
    assert!(!first.contains("plan.md"));
    assert_eq!(gitignore_with(&first, &[]), None);
    assert_eq!(
        gitignore_with("node_modules\n", &[]).unwrap(),
        format!("node_modules\n\n{first}")
    );
    assert_eq!(
        gitignore_with("node_modules", &[]).unwrap(),
        format!("node_modules\n{first}")
    );
    assert_eq!(
        gitignore_with("x\n\n", &[]).unwrap(),
        format!("x\n\n{first}")
    );
    assert_eq!(
        gitignore_with(&first, &[".dejafix/AI_INSTRUCTIONS.md.bak"]).unwrap(),
        format!("{first}\n.dejafix/AI_INSTRUCTIONS.md.bak\n")
    );
    let separated = ".dejafix/events.jsonl\u{2028}.dejafix/watch.pid\u{2028}.dejafix/watch.log\u{2028}.dejafix/structure.json";
    assert_eq!(
        gitignore_with(separated, &[]).unwrap(),
        format!("{separated}\n.djf-*\n")
    );
}

#[test]
fn a_plan_idea_goes_under_ideas_or_a_new_heading() {
    let plan = initial_plan("demo");
    let added = plan_with_idea(&plan, "  add an export adapter \n");
    let ideas = added
        .split("## Ideas")
        .nth(1)
        .unwrap()
        .split("##")
        .next()
        .unwrap();
    assert!(ideas.contains("- add an export adapter\n"));
    assert_eq!(
        plan_with_idea("# plan\n\n\u{a0}", "x"),
        "# plan\n\n## Ideas\n- x\n"
    );
    assert_eq!(plan_with_idea("", "x"), "\n\n## Ideas\n- x\n");
}

fn stack() -> Stack {
    Stack {
        tags: strings(&["javascript", "nextjs", "react"]),
        libraries: strings(&["next", "react"]),
        frameworks: strings(&["nextjs", "react"]),
        manifest_files: vec!["package.json"],
    }
}

#[test]
fn a_map_is_built_from_the_detected_stack() {
    let info = ManifestInfo {
        description: Some("A tiny Node app".to_owned()),
        entry_points: strings(&["`npm run start` → `node index.js`"]),
        main_folders: vec![
            ("src".to_owned(), "package or application code"),
            ("core".to_owned(), ""),
        ],
    };
    let map = project_map_from_stack("demo", &stack(), &info).unwrap();
    assert!(map.contains("auto-detected"));
    assert!(map.contains("## Project purpose\nA tiny Node app\n\n"));
    assert!(map.contains("- `src/` — package or application code\n- `core/`\n"));
    assert!(!map.contains("Status: not created yet"));
    insta::assert_snapshot!(map);
    assert_eq!(stack_summary_suffix(&stack()), " (nextjs, react)");
}

#[test]
fn nothing_detected_leaves_the_placeholder() {
    assert_eq!(
        project_map_from_stack("demo", &Stack::default(), &ManifestInfo::default()),
        None
    );
    let blank = ManifestInfo {
        description: Some(String::new()),
        ..ManifestInfo::default()
    };
    assert_eq!(
        project_map_from_stack("demo", &Stack::default(), &blank),
        None
    );
    let tagged = Stack {
        tags: strings(&["docker"]),
        manifest_files: vec!["Dockerfile"],
        ..Stack::default()
    };
    assert_eq!(stack_summary_suffix(&tagged), " (docker)");
    assert_eq!(stack_summary_suffix(&Stack::default()), "");
    let only_folders = ManifestInfo {
        main_folders: vec![("lib".to_owned(), "library code")],
        ..ManifestInfo::default()
    };
    let map = project_map_from_stack("demo", &Stack::default(), &only_folders).unwrap();
    assert!(!map.contains("## Stack"));
}
