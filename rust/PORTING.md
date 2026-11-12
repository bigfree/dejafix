# Porting the Python tests

Every test collected by `pytest --collect-only -q tests` has one row: the
milestone that ports it, or `out of scope` with the reason it has no Rust
counterpart. Parametrised cases are separate rows. When a milestone ports a
test, its row gets the Rust test path in the `Ported` column.

Regenerate the id list with
`.venv/bin/python -m pytest --collect-only -q tests | grep '::'` and keep this
table in step with it: a new Python test adds a row, a removed one drops its
row.

Rows: 512.

| Milestone | Tests |
|---|---|
| M1 | 24 |
| M2 | 187 |
| M3 | 149 |
| M4 | 59 |
| M5 | 17 |
| M6 | 14 |
| out of scope | 62 |

| Python test | Milestone | Scope | Ported |
|---|---|---|---|
| `tests/test_agent_guidance.py::test_the_bridge_names_the_project` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_the_bridge_is_still_valid_without_a_name` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_init_writes_the_registered_name_into_claude_md` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_supersedes_is_documented_on_every_surface[AI_INSTRUCTIONS.md (get_instructions)-ai_instructions]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_supersedes_is_documented_on_every_surface[CLAUDE.md bridge-<lambda>]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_supersedes_is_documented_on_every_surface[MCP instructions= field-_mcp_instructions]` | M4 | MCP server `instructions` | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[AI_INSTRUCTIONS.md (get_instructions)]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[CLAUDE.md bridge]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[MCP instructions= field]` | M4 | MCP server `instructions` | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[README.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[TUTORIAL.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[llms.txt]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[docs/2026-11-12-rust-m0-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[docs/2026-11-12-rust-m1-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[docs/2026-11-12-rust-m2-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_uses_the_tools_real_arguments[docs/2026-11-12-rust-rewrite-design.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[AI_INSTRUCTIONS.md (get_instructions)]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[CLAUDE.md bridge]` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[MCP instructions= field]` | M4 | MCP server `instructions` | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[README.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[TUTORIAL.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[llms.txt]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[docs/2026-11-12-rust-m0-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[docs/2026-11-12-rust-m1-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[docs/2026-11-12-rust-m2-plan.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_every_call_shown_passes_the_tools_required_arguments[docs/2026-11-12-rust-rewrite-design.md]` | M6 | user documentation is rewritten for 1.0 | |
| `tests/test_agent_guidance.py::test_precheck_file_is_shown_on_every_runtime_surface` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_the_call_scanner_reads_names_not_values` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_supersedes_guidance_says_what_it_does_to_the_summary` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_instructions_state_the_record_length_the_summary_clips_at` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_every_write_tool_states_its_record_length` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_the_three_surfaces_agree_about_the_session_start_order` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_get_context_drops_a_superseded_decision` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_precheck_does_not_warn_from_a_retired_event` | M3 | precheck skips superseded events | |
| `tests/test_agent_guidance.py::test_every_agent_facing_surface_filters_superseded` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_init_writes_the_bridge_to_both_rule_files` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_both_rule_files_say_exactly_the_same_thing` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_an_existing_agents_md_is_preserved` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_re_running_init_does_not_duplicate_the_block` | M2 | agent guidance surfaces | |
| `tests/test_agent_guidance.py::test_list_projects_leads_with_the_active_project` | M4 | MCP tools and instructions | |
| `tests/test_agent_guidance.py::test_instructions_do_not_tell_the_agent_to_list_projects_first` | M2 | agent guidance surfaces | |
| `tests/test_attempt.py::test_attempt_on_an_unknown_issue_is_refused` | M2 | write path: attempt and issue resolution | `crates/dejafix/tests/cli/write.rs::attempt_on_an_unknown_issue_is_refused_and_the_project_still_works` |
| `tests/test_attempt.py::test_attempt_issue_id_is_normalised_like_fix` | M2 | write path: attempt and issue resolution | `crates/dejafix/tests/cli/write.rs::attempt_issue_id_is_normalised_like_fix` |
| `tests/test_attempt.py::test_a_stale_current_issue_marker_is_ignored` | M2 | write path: attempt and issue resolution | `crates/dejafix/tests/cli/write.rs::a_stale_current_issue_marker_is_ignored` |
| `tests/test_attempt.py::test_fix_ignores_and_clears_a_stale_marker` | M2 | write path: attempt and issue resolution | `crates/dejafix/tests/cli/write.rs::fix_ignores_and_clears_a_stale_marker` |
| `tests/test_attempt.py::test_a_bare_hash_is_reported_readably` | M2 | write path: attempt and issue resolution | `crates/dejafix/tests/cli/write.rs::a_bare_hash_is_reported_readably` |
| `tests/test_auto_capture.py::test_location_skips_dejafix_files` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_memory_only_commit_has_no_location` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_deleted_file_is_never_the_location` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_rename_is_located_at_the_new_path` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_the_repos_first_commit_lists_its_files` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_non_ascii_path_is_stored_as_written` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_code_outranks_docs_and_ci_config` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_docs_outrank_lockfiles_and_a_tie_keeps_git_order` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_manifests_and_build_files_never_beat_source` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_first_commit_with_manifests_is_located_at_the_code` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_source_outranks_tests_which_outrank_other_files` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_only_a_top_level_dot_dir_or_dot_file_counts_as_config` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_merge_commit_records_the_files_it_brought_in` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_memory_files_come_last_so_the_cap_keeps_the_code` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_the_location_is_always_first_in_files` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_a_code_file_whose_name_starts_like_a_doc_or_manifest_is_code` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_explicit_build_code_files_are_still_demoted` | M3 | git auto-capture | |
| `tests/test_auto_capture.py::test_binary_assets_do_not_beat_docs` | M3 | git auto-capture | |
| `tests/test_client_config_discovery.py::test_a_store_installed_claude_desktop_is_discovered` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_the_store_entry_is_labelled_distinctly` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_appdata_is_honoured_over_the_default_under_home` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_several_store_packages_are_all_found_in_a_stable_order` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_no_packages_directory_is_not_an_error` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_an_unreadable_config_is_reported_rather_than_skipped` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_a_missing_config_says_nothing_at_all` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_a_project_scoped_mcp_json_is_checked` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_client_config_discovery.py::test_a_clean_project_config_is_not_flagged` | M2 | client config discovery, used by `doctor` (M2) and `migrate` (M5) | |
| `tests/test_dashboard.py::test_register_project_is_idempotent` | M2 | project registry | |
| `tests/test_dashboard.py::test_init_registers_the_project` | M2 | project registry | |
| `tests/test_dashboard.py::test_registered_projects_skips_deleted_memory` | M2 | project registry | |
| `tests/test_dashboard.py::test_dashboard_with_no_projects_exits_cleanly` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_dashboard_generates_index_and_per_project_dashboards` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_dashboard_back_link_injected_into_project_dashboards` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_dashboard_open_and_fix_totals` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_dashboard_never_writes_to_the_projects` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_static_mode_marks_the_page_as_a_snapshot` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_serve_mode_marks_the_page_as_live` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_back_linked_injects_before_body_and_is_safe_without_one` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_dashboard.py::test_dashboard_output_follows_dejafix_home` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input0-expected0]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input1-expected1]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input2-expected2]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input3-expected3]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input4-expected4]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input5-expected5]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[tool_input6-expected6]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_edited_paths_reads_claude_and_codex_shapes[not a dict-expected7]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[project.json: keep targets sorted-True]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[see project.json.-True]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[(project.json)-True]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[libs/db/project.json pins the schema-False]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[old-project.json is gone-False]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[project.json5 is different-False]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_mention_pattern_respects_path_boundaries[project.jsonl is different-False]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_claude_code_edit_gets_issue_attempt_and_decision` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_codex_patch_paths_resolve_against_cwd` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_codex_patch_reports_each_file_with_lessons` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_codex_patch_across_two_projects_reads_each_log` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_silent_for_file_without_lessons` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_silent_outside_any_project` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_shown_once_per_session_until_lessons_change` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_subagent_sees_lessons_its_parent_already_saw` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_shows_lessons_again[event0]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_shows_lessons_again[event1]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_leaves_other_sessions_alone` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_without_session_id_every_edit_gets_lessons` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_shared_dedup_directory_is_not_trusted` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_symlinked_dedup_directory_is_not_followed` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_worktree_edit_reads_main_checkout_memory` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_worktree_with_its_own_memory_falls_back_to_it` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_memory_and_agent_files_are_skipped[.dejafix/events.jsonl]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_memory_and_agent_files_are_skipped[.claude/settings.json]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_summary_mention_counts_only_whole_paths` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_decision_with_its_own_files_ignores_passing_mentions` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_event_paths_in_other_spellings_still_match[src\\b.py]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_event_paths_in_other_spellings_still_match[./src/../src/b.py]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_event_paths_in_other_spellings_still_match[src/b.py:3:7]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_event_paths_in_other_spellings_still_match[{root}/src/b.py]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_event_path_through_symlinked_root_still_matches` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_closed_issue_and_superseded_decision_are_dropped` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_git_captured_decision_is_left_out` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_attempt_inherits_its_issue_location` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_long_history_is_capped` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_attempt_without_issue_renders_without_empty_reference` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_injected_issue_id_does_not_reach_context` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_undecodable_and_broken_event_lines_are_skipped` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_absolute_event_path_outside_the_project_is_ignored` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_broken_dedup_storage_still_shows_lessons` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_without_dedup_state_is_a_no_op[]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_without_dedup_state_is_a_no_op[s1]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_compaction_never_clears_a_shared_directory` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_prints_one_json_line` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[not json]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[[]]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[{}]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[{"tool_input": "x"}]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[\xff\xfe]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_on_malformed_input[{"session_id": "s", "tool_input": {"file_path": "a\u0000b"}}]` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_main_is_silent_when_the_log_is_unreadable` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_module_entry_point_runs_as_a_hook` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_unusable_temp_dir_falls_back_to_showing_lessons` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_call_log_records_each_call_without_lesson_text` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_call_log_records_the_error_the_hook_swallowed` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_unwritable_call_log_is_ignored` | M3 | edit hook | |
| `tests/test_edit_hook.py::test_no_call_log_without_the_variable` | M3 | edit hook | |
| `tests/test_global_mcp.py::test_a_0_2_x_registry_is_read_without_being_rewritten` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_dashboard_still_reads_the_registry_after_migration` | out of scope | `djf dashboard` is removed in 1.0 | |
| `tests/test_global_mcp.py::test_ids_do_not_collide_for_same_named_folders` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_unknown_project_errors_with_the_known_ones` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_explicit_selection_wins` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_active_project_is_used_when_no_selection` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_client_roots_outrank_a_stale_active_project` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_two_usable_roots_is_an_error_not_a_coin_flip` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_nothing_selected_refuses_rather_than_guessing` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_pinned_root_is_a_boundary_not_a_default` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_project_cli_round_trip` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_registering_an_uninitialised_folder_is_refused` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_one_server_writes_to_the_project_it_was_told` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_write_without_a_project_fails_visibly` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_a_single_registered_project_needs_no_selection` | M4 | MCP project resolution | |
| `tests/test_global_mcp.py::test_an_older_dejafix_can_only_append_a_path` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_a_dev_format_registry_is_converted_and_backed_up` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_windows_paths_survive_migration` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_absolute_check_is_used_rather_than_a_slash_test` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_a_utf8_bom_does_not_empty_the_registry` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_a_utf8_bom_does_not_break_the_event_log` | M1 | UTF-8 BOM in events.jsonl | `crates/dejafix-core/src/core/log/tests.rs::a_utf8_bom_does_not_break_the_event_log` |
| `tests/test_global_mcp.py::test_scan_finds_projects_the_registry_never_knew_about` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_scan_is_idempotent` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_scan_accepts_several_roots` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_package_version_matches_pyproject` | M5 | version in Cargo and the maturin wheel match | |
| `tests/test_global_mcp.py::test_doctor_registers_what_it_finds` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_prunes_entries_whose_memory_is_gone` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_upgrade_notice_fires_once` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_no_notice_on_a_first_ever_run` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_default_depth_reaches_a_nested_repo_folder` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_cloud_folders_are_included_by_default` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_cloud_roots_are_deduped_by_what_they_resolve_to` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_several_cloud_clients_are_covered` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_project_lookup_is_case_insensitive` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_the_same_folder_reached_twice_registers_once` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_dedupe_paths_uses_identity_not_text` | M2 | project registry and `djf project` | |
| `tests/test_global_mcp.py::test_no_network_unless_asked` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_online_flag_reports_a_newer_release` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_auto_check_is_remembered_and_can_be_turned_off` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_version_comparison_ignores_suffixes` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_remembers_a_config_that_was_clean_and_is_pinned_again` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_does_not_cry_revert_for_a_config_pinned_all_along` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_state_survives_an_unwritable_home` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_tells_you_to_quit_the_client_before_editing` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_records_a_clean_config_it_has_never_seen_before` | M2 | `djf doctor` | |
| `tests/test_global_mcp.py::test_doctor_ignores_configs_that_never_mention_dejafix` | M2 | `djf doctor` | |
| `tests/test_hooks_path.py::test_resolve_djf_binary_returns_absolute_path_or_bare_name` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_precheck_snippet_bakes_path_into_script` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_auto_capture_snippet_bakes_path_and_capture_arg` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_hook_runs_under_stripped_path` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_install_hooks_uses_resolved_path` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_resolver_prefers_the_djf_next_to_the_interpreter` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_resolver_falls_back_to_path_without_an_adjacent_djf` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_resolver_skips_an_adjacent_djf_that_is_not_executable` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_resolver_finds_scripts_djf_exe_on_windows` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_resolver_with_nothing_found_is_the_bare_name` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_a_hook_baked_by_an_old_release_gets_the_new_path_and_shebang` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_a_users_own_lines_and_shebang_survive_a_refresh_byte_for_byte` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_a_second_install_changes_nothing_and_says_so` | M3 | git hook install and binary path | |
| `tests/test_hooks_path.py::test_a_block_with_no_end_marker_is_left_alone_with_a_warning` | M3 | git hook install and binary path | |
| `tests/test_init.py::test_init_creates_dejafix_files` | M2 | `djf init`, `instructions`, `map` | |
| `tests/test_init.py::test_instructions_command_prints_ai_protocol` | M2 | `djf init`, `instructions`, `map` | |
| `tests/test_init.py::test_map_command_prints_project_map` | M2 | `djf init`, `instructions`, `map` | |
| `tests/test_init_ux.py::test_stack_detect_does_nothing_when_no_manifests` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_stack_detect_pyproject_populates_map` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_stack_detect_package_json_populates_map` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_stack_detect_never_overwrites_user_edited_map` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_stack_detect_handles_corrupt_manifest` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_extract_description_pyproject` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_extract_description_package_json` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_extract_description_returns_none_when_absent` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_extract_entry_points_pyproject_scripts` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_extract_entry_points_npm_scripts` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_detect_main_folders` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_print_mcp_config_contains_expected_pieces` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_print_mcp_config_single_project_still_pins_the_root` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_print_mcp_config_uses_absolute_python` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_init_warns_when_a_client_config_still_pins_one_repo` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_init_detects_a_pin_set_through_the_environment` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_init_prints_the_codex_toml_form` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_linux_client_paths_are_scanned_and_xdg_is_honoured` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_printed_locations_match_the_platform` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_init_ux.py::test_windows_locations_use_windows_conventions` | M2 | `djf init` stack detection and MCP config output | |
| `tests/test_instructions_refresh.py::test_the_template_carries_the_current_marker` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_a_fresh_init_writes_the_current_file_without_a_backup` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_init_refreshes_a_stale_file_keeping_a_backup_and_the_global_block` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_init_leaves_a_current_file_with_user_edits_untouched` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_an_older_numbered_marker_is_refreshed_too` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_a_refresh_with_no_global_block_adds_none` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_initialize_alone_never_rewrites_the_file` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_instructions_refresh.py::test_a_template_change_comes_with_a_version_bump` | M2 | AI_INSTRUCTIONS.md template version 3 | |
| `tests/test_location_parsing.py::test_location_to_file[src/foo.py-src/foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[src/foo.py:42-src/foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[src/foo.py:42:7-src/foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[foo.py-foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[deep/nested/path/mod.ts:1-deep/nested/path/mod.ts]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[C:\\src\\foo.py-C:\\src\\foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[C:\\src\\foo.py:42-C:\\src\\foo.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[D:/repo/app.tsx:9-D:/repo/app.tsx]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[src\\windows\\rel.py:3-src\\windows\\rel.py]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[AuthService.validate-None]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[SomeClass-None]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[-None]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_location_to_file[None-None]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases (`None` is `Option::None` at the call site)` |
| `tests/test_location_parsing.py::test_location_to_file[   -None]` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::location_to_file_cases` |
| `tests/test_location_parsing.py::test_a_bare_path_counts_the_same_as_one_with_a_line_number` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::a_bare_path_counts_the_same_as_one_with_a_line_number (score half: M4 `get_score`)` |
| `tests/test_location_parsing.py::test_a_windows_path_is_not_counted_as_a_file_called_c` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::a_windows_path_is_not_counted_as_a_file_called_c (score half: M4 `get_score`)` |
| `tests/test_location_parsing.py::test_a_method_location_is_not_a_file` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::a_method_location_is_not_a_file (score half: M4 `get_score`)` |
| `tests/test_location_parsing.py::test_the_same_file_by_two_spellings_counts_once` | M1 | location and path normalisation | `crates/dejafix-core/src/core/path/tests.rs::the_same_file_by_two_spellings_counts_once (score half: M4 `get_score`)` |
| `tests/test_location_parsing.py::test_global_dir_follows_dejafix_home` | M2 | global memory follows DEJAFIX_HOME | `crates/dejafix/tests/cli/write.rs::appending_an_event_stays_inside_an_isolated_home` |
| `tests/test_location_parsing.py::test_global_dir_falls_back_to_home_without_the_env_var` | M2 | global memory follows DEJAFIX_HOME | `crates/dejafix/tests/cli/write.rs::global_memory_falls_back_to_home_without_the_env_var` |
| `tests/test_location_parsing.py::test_appending_an_event_stays_inside_an_isolated_home` | M2 | global memory follows DEJAFIX_HOME | `crates/dejafix/tests/cli/write.rs::appending_an_event_stays_inside_an_isolated_home` |
| `tests/test_log.py::test_log_attempt_and_fix_write_events` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::log_attempt_and_fix_write_events` |
| `tests/test_log.py::test_fix_issue_closes_target_without_clearing_newer_active_issue` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::fix_issue_closes_target_without_clearing_newer_active_issue` |
| `tests/test_log.py::test_plain_fix_still_closes_active_issue` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::plain_fix_still_closes_active_issue` |
| `tests/test_log.py::test_fix_issue_rejects_unknown_issue` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::fix_issue_rejects_unknown_issue` |
| `tests/test_log.py::test_mcp_record_fix_accepts_target_issue_id` | M4 | MCP `record_fix` | |
| `tests/test_log.py::test_attempt_without_open_issue_fails` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::attempt_without_open_issue_fails` |
| `tests/test_log.py::test_attempt_auto_issue_creates_parent` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::attempt_auto_issue_creates_parent` |
| `tests/test_log.py::test_attempt_after_fix_does_not_misattribute` | M2 | write path: log, attempt, fix | `crates/dejafix/tests/cli/write.rs::attempt_after_fix_does_not_misattribute` |
| `tests/test_mcp_server.py::test_git_helpers_detach_stdin_and_keep_timeouts` | M3 | git adapter: detached stdin and timeouts | |
| `tests/test_mcp_server.py::test_mcp_stdio_precheck_file_returns_with_git_backed_analysis` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_precheck_file_names_a_memory_file_instead_of_clearing_it` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_stdio_add_note_returns_and_writes_event` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_server_instantiation` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_server_builds_when_fastmcp_is_gone` | out of scope | Python MCP SDK packaging; rmcp has no such fallback | |
| `tests/test_mcp_server.py::test_mcp_precheck_file_matches_every_spelling_of_a_path[src/auth.py]` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_precheck_file_matches_every_spelling_of_a_path[./src/auth.py]` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_precheck_file_matches_every_spelling_of_a_path[ABSOLUTE]` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_precheck_file_matches_an_event_logged_with_an_absolute_path` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_get_issue_accepts_unpadded_ids_and_rejects_globs` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_get_issue_finds_backfilled_legacy_ids` | M4 | MCP server | |
| `tests/test_mcp_server.py::test_mcp_precheck_files_matches_single_calls` | M4 | MCP server | |
| `tests/test_migrate.py::test_dry_run_changes_nothing` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_apply_moves_memory_home_and_hooks` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_second_run_is_a_no_op` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_existing_dejafix_folder_is_never_overwritten` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_project_with_a_running_old_watcher_is_skipped` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_home_stamped_by_a_first_djf_run_is_replaced` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_home_entry_with_data_in_both_places_is_kept` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_unregistered_project_given_by_path` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_stale_client_configs_are_reported` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_dry_run_does_not_repair_the_legacy_registry` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_run_that_stopped_after_the_rename_is_finished` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_bridge_already_added_by_djf_init_is_not_duplicated` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_relative_path_gets_the_folder_name` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_symlinked_memory_folder_is_left_alone` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_unregistered_home_data_is_not_deleted` | M5 | `djf migrate` | |
| `tests/test_migrate.py::test_a_folder_named_projectmem_is_not_a_stale_config` | M5 | `djf migrate` | |
| `tests/test_models.py::test_an_event_without_an_id_gets_the_same_id_on_every_read` | M1 | event model and stable ids | `crates/dejafix-core/src/core/event/tests.rs::an_event_without_an_id_gets_the_same_id_on_every_read` |
| `tests/test_models.py::test_events_without_ids_but_different_content_get_different_ids` | M1 | event model and stable ids | `crates/dejafix-core/src/core/event/tests.rs::events_without_ids_but_different_content_get_different_ids` |
| `tests/test_models.py::test_a_stored_id_is_kept` | M1 | event model and stable ids | `crates/dejafix-core/src/core/event/tests.rs::a_stored_id_is_kept` |
| `tests/test_plan.py::test_init_creates_plan_md` | M2 | plan.md and `djf plan` | |
| `tests/test_plan.py::test_plan_md_is_committed_not_gitignored` | M2 | plan.md and `djf plan` | |
| `tests/test_plan.py::test_djf_plan_add_appends_to_ideas` | M2 | plan.md and `djf plan` | |
| `tests/test_plan.py::test_plan_add_does_not_touch_events` | M2 | plan.md and `djf plan` | |
| `tests/test_plan.py::test_get_plan_mcp_reads_the_file` | M4 | MCP `get_plan` | |
| `tests/test_precheck_lessons.py::test_attempt_without_location_inherits_its_issue_file` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_partial_attempt_is_a_lesson` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_failure_older_than_thirty_days_still_warns` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_a_worked_attempt_is_not_a_lesson` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_lesson_title_counts_partial_attempts_separately` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_overflow_names_the_issues_of_hidden_lessons` | M3 | precheck lessons | |
| `tests/test_precheck_lessons.py::test_precheck_files_analyzes_all_paths_in_one_pass` | M3 | precheck lessons | |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-openai_key]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-github_token]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-github_pat]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-aws_access_key]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-google_api_key]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-slack_token]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-stripe_key0]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-stripe_key1]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-jwt]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-bearer_token]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `tests/test_redaction.py::test_redacts_known_secret_shapes[<lambda>-private_key]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redacts_known_secret_shapes` |
| `` tests/test_redaction.py::test_does_not_touch_ordinary_prose[tried `contain: layout` in styles.css and it did not fix the layout shift] `` | M2 | redaction before write |`crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[summary: bug in src/auth.py \u2014 login form submits twice]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[Decision: use bcrypt rounds=12 for password hashing.]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[OPENAI_API_KEY is required in the environment for this script to run.]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[.dejafix/events.jsonl]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[sk- short prefix only]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[ghp_short]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[Forgot password reset flow needs a token email.]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[use bearer auth]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_does_not_touch_ordinary_prose[version eyJsomeshortstring]` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::does_not_touch_ordinary_prose` |
| `tests/test_redaction.py::test_is_redaction_enabled_default_on` | M2 | redaction before write | `crates/dejafix/tests/cli/write.rs::secrets_are_redacted_with_a_notice_unless_disabled` |
| `tests/test_redaction.py::test_env_var_disables[1]` | M2 | redaction before write | `crates/dejafix/tests/cli/write.rs::secrets_are_redacted_with_a_notice_unless_disabled` |
| `tests/test_redaction.py::test_env_var_disables[true]` | M2 | redaction before write | `crates/dejafix/tests/cli/write.rs::secrets_are_redacted_with_a_notice_unless_disabled` |
| `tests/test_redaction.py::test_env_var_disables[yes]` | M2 | redaction before write | `crates/dejafix/tests/cli/write.rs::secrets_are_redacted_with_a_notice_unless_disabled` |
| `tests/test_redaction.py::test_redact_event_fields_honours_opt_out` | M2 | redaction before write | `crates/dejafix-core/src/core/write/tests.rs::secrets_in_summary_and_location_are_redacted_unless_disabled` |
| `tests/test_redaction.py::test_redact_event_fields_default_on` | M2 | redaction before write | `crates/dejafix-core/src/core/write/tests.rs::secrets_in_summary_and_location_are_redacted_unless_disabled` |
| `tests/test_redaction.py::test_redactable_fields_constant_is_stable` | M2 | redaction before write | `crates/dejafix-core/src/core/redact/tests.rs::redactable_fields_constant_is_stable` |
| `tests/test_search.py::test_search_finds_matching_events` | M2 | search | `crates/dejafix/tests/cli/read.rs::search_finds_matching_events` |
| `tests/test_session_issue.py::test_open_issue_ids_are_newest_first_without_fixed` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_issue.py::test_sole_open_issue_resolves_the_only_open_issue` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_issue.py::test_sole_open_issue_is_none_without_open_issues` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_issue.py::test_sole_open_issue_refuses_and_lists_every_open_issue` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_issue.py::test_refusal_caps_the_listing` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_issue.py::test_two_mcp_processes_do_not_misfile` | M4 | concurrent MCP writers | |
| `tests/test_session_issue.py::test_one_open_issue_still_needs_no_id` | M2 | open-issue resolution | `crates/dejafix-core/src/core/write/tests.rs::several_open_issues_are_listed_newest_first_and_capped` |
| `tests/test_session_start.py::test_start_session_contains_rules_summary_and_active_plan` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_omits_ideas_shipped_and_someday` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_tolerates_missing_files` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_server_instructions_name_start_session_and_allow_parallel_calls` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_keeps_custom_plan_headings` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_reads_a_plan_that_starts_with_a_heading` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_survives_an_empty_trailing_heading` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_says_the_session_start_reads_are_done` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_includes_a_placeholder_map_for_setup_mode` | M4 | MCP `start_session` | |
| `tests/test_session_start.py::test_start_session_leaves_out_a_real_map` | M4 | MCP `start_session` | |
| `tests/test_showoff.py::test_template_has_showoff_nav_and_panel` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_showoff.py::test_template_has_all_three_scenes` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_showoff.py::test_template_has_recorder_controls` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_showoff.py::test_template_has_watermark_toggle` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_showoff.py::test_showoff_uses_existing_data_only` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_showoff.py::test_graph_data_feeds_showoff_scenes` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_staleness_performance.py::test_counts_match_a_per_event_git_log` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_one_git_call_per_file_not_per_event` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_only_files_restricts_the_work_and_the_answer` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_a_deleted_file_flags_without_asking_git` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_git_failure_reads_as_cannot_judge_not_stale` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_threshold_is_still_respected` | M3 | staleness performance | |
| `tests/test_staleness_performance.py::test_superseded_events_are_still_skipped` | M3 | staleness performance | |
| `tests/test_staleness_repair.py::test_commit_counts_do_not_depend_on_committer_or_machine_offsets[UTC]` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_commit_counts_do_not_depend_on_committer_or_machine_offsets[America/Denver]` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_commit_counts_do_not_depend_on_committer_or_machine_offsets[Asia/Dhaka]` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_commit_times_are_canonical_zulu` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_an_old_event_located_at_summary_md_is_not_flagged` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_precheck_skips_a_staged_summary_md` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_an_auto_captured_fix_or_note_is_not_flagged_as_stale[fix]` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_an_auto_captured_fix_or_note_is_not_flagged_as_stale[note]` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_an_auto_captured_decision_is_still_a_claim_and_is_flagged` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_memory_path_rule_is_anchored_at_the_project_root` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_memory_path_rule_folds_dot_dot_and_anchors_a_subdir_cwd` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_precheck_and_staleness_agree_on_a_nested_dejafix_dir` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_a_manual_decision_in_the_same_spot_is_still_flagged` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_an_absolute_path_through_a_symlinked_folder_is_still_ours` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_a_decision_logged_at_an_absolute_path_is_stale_for_precheck_too` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_a_windows_drive_path_is_made_project_relative` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_precheck_matches_a_windows_drive_path_on_a_windows_root` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_two_spellings_of_one_file_warn_once` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_the_project_root_is_not_a_file` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_a_blank_path_is_not_a_file` | M3 | staleness | |
| `tests/test_staleness_repair.py::test_precheck_normalizes_each_logged_path_once` | M3 | staleness | |
| `tests/test_structure.py::test_walk_finds_source_and_excludes_noise` | out of scope | the structure.json cache is removed in 1.0 | |
| `tests/test_structure.py::test_python_imports_resolve_to_internal_files_only` | out of scope | the structure.json cache is removed in 1.0 | |
| `tests/test_structure.py::test_write_structure_writes_gitignored_cache` | out of scope | the structure.json cache is removed in 1.0 | |
| `tests/test_structure.py::test_extractor_never_touches_events` | out of scope | the structure.json cache is removed in 1.0 | |
| `tests/test_summary.py::test_summary_and_issue_file_are_regenerated` | M2 | summary.md and issue files | `crates/dejafix-core/src/wiring/tests.rs::log_attempt_fix_keep_the_marker_and_issue_file_in_step` |
| `tests/test_summary.py::test_regenerate_preserves_project_purpose` | M2 | summary.md and issue files | `crates/dejafix/tests/golden/m2.rs::purpose_extraction_matches_python` |
| `tests/test_summary.py::test_project_purpose_auto_syncs_from_project_map` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::purpose_comes_from_the_map_then_the_summary` |
| `tests/test_summary.py::test_project_purpose_falls_back_when_project_map_is_placeholder` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::purpose_comes_from_the_map_then_the_summary` |
| `tests/test_summary.py::test_recent_issues_capped_at_ten` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::recent_issues_capped_at_ten` |
| `tests/test_summary.py::test_recent_issues_under_ten_all_listed` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::recent_issues_under_ten_all_listed` |
| `tests/test_summary.py::test_an_old_open_issue_is_never_capped_away` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::an_old_open_issue_is_never_capped_away` |
| `tests/test_summary.py::test_many_open_issues_are_all_listed` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::many_open_issues_are_all_listed` |
| `tests/test_summary.py::test_a_large_open_backlog_is_named_not_dropped` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_large_open_backlog_is_named_not_dropped` |
| `tests/test_summary.py::test_issue_ids_past_9999_order_numerically` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::issue_ids_past_9999_order_numerically` |
| `tests/test_summary.py::test_decisions_capped_at_newest_fifteen` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::decisions_capped_at_newest_fifteen` |
| `tests/test_summary.py::test_an_orphan_attempt_does_not_crash_the_summary` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::an_orphan_attempt_does_not_crash_the_summary` |
| `tests/test_summary.py::test_a_fixed_issue_is_listed_by_its_title_alone[auth/redirect.py-auth/middleware.py- [auth/redirect.py]]` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_fixed_issue_is_listed_by_its_title_alone` |
| `tests/test_summary.py::test_a_fixed_issue_is_listed_by_its_title_alone[None-auth/middleware.py- [auth/middleware.py]]` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_fixed_issue_is_listed_by_its_title_alone` |
| `tests/test_summary.py::test_a_fixed_issue_is_listed_by_its_title_alone[None-None-]` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_fixed_issue_is_listed_by_its_title_alone` |
| `tests/test_summary.py::test_long_records_are_clipped_but_keep_their_location` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::long_records_are_clipped_but_keep_their_location` |
| `tests/test_summary.py::test_a_long_path_is_not_clipped_down_to_the_word_before_it` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_long_path_is_not_clipped_down_to_the_word_before_it` |
| `tests/test_summary.py::test_a_multi_line_record_stays_on_one_summary_line` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_multi_line_record_stays_on_one_summary_line` |
| `tests/test_summary.py::test_a_blank_location_falls_back_to_the_fixs` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::a_blank_location_falls_back_to_the_fixs` |
| `tests/test_summary.py::test_key_files_ignore_version_numbers_and_identifiers` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::key_files_ignore_version_numbers_and_identifiers` |
| `tests/test_summary.py::test_key_files_keep_root_level_files_mentioned_in_text` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::key_files_keep_root_level_files_mentioned_in_text` |
| `tests/test_summary.py::test_key_files_include_locations_that_name_a_file` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::key_files_include_locations_that_name_a_file` |
| `tests/test_summary.py::test_key_files_merge_line_suffixes_into_one_file` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::key_files_merge_line_suffixes_into_one_file` |
| `tests/test_summary.py::test_key_files_rank_by_event_count_and_skip_memory_files` | M2 | summary.md and issue files | `crates/dejafix-core/src/core/summary/tests.rs::key_files_rank_by_event_count_and_skip_memory_files` |
| `tests/test_tool_replies.py::test_add_note_reply_names_the_event_instead_of_echoing_it` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_add_decision_reply_names_the_event_and_what_it_retires` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_issue_writes_reply_with_the_issue_id_only` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_clips_long_matches_and_names_each_event` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_full_returns_whole_records` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_add_decision_reply_resolves_a_supersedes_prefix` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_marks_superseded_decisions` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_shows_the_part_that_matched_past_the_clip` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_shows_a_match_found_only_in_notes` | M4 | MCP reply texts | |
| `tests/test_tool_replies.py::test_search_full_keeps_one_match_per_line` | M4 | MCP reply texts | |
| `tests/test_v014_features.py::test_supersede_hides_old_decision_from_summary` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/write.rs::supersede_hides_old_decision_from_summary_and_accepts_a_prefix` |
| `tests/test_v014_features.py::test_supersede_accepts_unique_prefix` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/write.rs::supersede_hides_old_decision_from_summary_and_accepts_a_prefix` |
| `tests/test_v014_features.py::test_supersede_bad_ref_fails_without_writing` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/write.rs::supersede_bad_ref_fails_without_writing` |
| `tests/test_v014_features.py::test_resolve_event_ref_ambiguous_and_missing` | M1 | event reference and location parsing | `crates/dejafix-core/src/core/log/tests.rs::resolve_event_ref_ambiguous_and_missing` |
| `tests/test_v014_features.py::test_search_tags_superseded` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/read.rs::search_tags_superseded_and_shows_ids` |
| `tests/test_v014_features.py::test_stale_flags_decision_after_commits` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_stale_flags_missing_file_and_skips_superseded` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_location_file_ignores_non_paths` | M1 | event reference and location parsing | `crates/dejafix-core/src/core/path/tests.rs::location_file_ignores_non_paths` |
| `tests/test_v014_features.py::test_parse_snooze_duration` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_snooze_silences_and_unsnooze_restores` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_expired_snooze_self_cleans` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_brief_shows_all_sections` | M4 | `djf brief` needs staleness (M3) and score (M4) | |
| `tests/test_v014_features.py::test_brief_runs_clean_on_fresh_project` | M4 | `djf brief` needs staleness (M3) and score (M4) | |
| `tests/test_v014_features.py::test_brief_console_helpers_are_cp1252_safe` | M4 | `djf brief` needs staleness (M3) and score (M4) | |
| `tests/test_v014_features.py::test_precheck_lists_failed_approaches` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_precheck_console_helpers_are_cp1252_safe` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_precheck_named_files_cli` | M3 | precheck, staleness and snooze | |
| `tests/test_v014_features.py::test_search_matches_location_field` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/read.rs::search_matches_location_and_filters_failed_attempts` |
| `tests/test_v014_features.py::test_search_failed_only` | M2 | supersede, brief, search | `crates/dejafix/tests/cli/read.rs::search_matches_location_and_filters_failed_attempts` |
| `tests/test_v014_features.py::test_export_writes_marked_block` | out of scope | `djf export` is removed in 1.0 | |
| `tests/test_v014_features.py::test_export_is_idempotent_and_preserves_user_content` | out of scope | `djf export` is removed in 1.0 | |
| `tests/test_v014_features.py::test_export_excludes_superseded_and_flags_stale` | out of scope | `djf export` is removed in 1.0 | |
| `tests/test_v014_features.py::test_export_cursor_variant` | out of scope | `djf export` is removed in 1.0 | |
| `tests/test_v014_features.py::test_walkup_discovery_skips_global_store` | M2 | project discovery | `crates/dejafix/tests/cli/read.rs::a_subdirectory_finds_the_project_but_the_global_store_is_never_a_project` |
| `tests/test_v014_features.py::test_walkup_discovery_still_finds_real_projects` | M2 | project discovery | `crates/dejafix/tests/cli/read.rs::a_subdirectory_finds_the_project_but_the_global_store_is_never_a_project` |
| `tests/test_visualize.py::test_location_path_for_graph_accepts_file_path_without_line` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_location_path_for_graph_accepts_file_path_with_line` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_location_path_for_graph_normalizes_windows_separators` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_location_path_for_graph_accepts_existing_directory` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_location_path_for_graph_rejects_descriptive_locations` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_adds_file_directory_metadata` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_groups_root_level_files_under_root_bucket` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_counts_all_attached_events_for_dense_files` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_marks_nine_events_as_not_dense` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_importance_includes_failure_weight` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_counts_same_file_and_location_once_per_event` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_deduplicates_duplicate_file_links` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_links_path_like_location_without_line` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_keeps_descriptive_location_unlinked` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_counts_failed_attempts_for_path_like_location` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_still_links_explicit_files` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_build_graph_data_still_links_location_with_line` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_template_has_project_map_flow_view` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_template_has_timeline_spine_view` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_viz_data_cannot_close_the_script_tag` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_viz_embeds_hostile_text_escaped` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_viz_renders_event_text_through_the_escaper` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_visualize.py::test_json_for_script_escapes_tag_characters` | out of scope | `djf visualize` is removed in 1.0 | |
| `tests/test_watch.py::test_watch_status_not_running` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_watch_stop_not_running` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_watch_daemon_spawns_subprocess` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_pid_alive_sees_a_running_process_on_windows` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_pid_alive_reports_dead_for_a_missing_process_on_windows` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_pid_alive_treats_access_denied_as_alive` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_running_pid_keeps_the_pid_file_for_a_live_daemon_on_windows` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_running_pid_cleans_up_a_genuinely_stale_pid_file` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_terminate_uses_terminateprocess_on_windows` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_terminate_raises_process_lookup_for_a_missing_pid` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_watch.py::test_pid_alive_on_posix_reports_this_process` | out of scope | `djf watch` is removed in 1.0 | |
| `tests/test_windows_console_encoding.py::test_init_survives_a_cp1252_console` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_init_prints_the_mcp_block_on_cp1252` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_commands_survive_a_cp1252_console[command0]` | out of scope | `djf score` is removed in 1.0 | |
| `tests/test_windows_console_encoding.py::test_commands_survive_a_cp1252_console[command1]` | M2 | CLI output on a cp1252 console | `crates/dejafix/tests/cli/read.rs::output_is_utf8_without_replacement_characters` |
| `tests/test_windows_console_encoding.py::test_commands_survive_a_cp1252_console[command2]` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_no_replacement_characters_leak_into_cp1252_output` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_configure_stdio_tolerates_a_stream_it_cannot_reconfigure` | out of scope | Python stream wrapper; Rust writes UTF-8 through std | |
| `tests/test_windows_console_encoding.py::test_cp1252_output_contains_no_byte_that_renders_as_a_black_diamond` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_utf8_output_keeps_its_real_characters` | M2 | CLI output on a cp1252 console | |
| `tests/test_windows_console_encoding.py::test_folding_stream_still_looks_like_a_stream` | out of scope | Python stream wrapper; Rust writes UTF-8 through std | |
| `tests/test_windows_hooks_and_context.py::test_hook_shebang_is_posix_sh_not_bash` | M3 | Windows hooks | |
| `tests/test_windows_hooks_and_context.py::test_baked_binary_path_has_no_backslashes` | M3 | Windows hooks | |
| `tests/test_windows_hooks_and_context.py::test_baked_path_survives_a_real_shell` | M3 | Windows hooks | |
| `tests/test_windows_hooks_and_context.py::test_windows_falls_back_to_scripts_not_bin` | M3 | Windows hooks | |
| `tests/test_windows_hooks_and_context.py::test_installed_hook_does_not_block_a_commit` | M3 | Windows hooks | |
| `tests/test_windows_hooks_and_context.py::test_get_context_uses_the_resolved_project_not_cwd` | M4 | MCP tools use the resolved project | |
| `tests/test_windows_hooks_and_context.py::test_precheck_file_uses_the_resolved_project_not_cwd` | M4 | MCP tools use the resolved project | |
| `tests/test_windows_hooks_and_context.py::test_precheck_churn_counts_the_projects_git_not_the_servers` | M4 | MCP tools use the resolved project | |
| `tests/test_windows_hooks_and_context.py::test_an_expected_error_prints_no_traceback` | M2 | CLI error output | `crates/dejafix/tests/cli/write.rs::an_expected_error_prints_no_traceback` |
