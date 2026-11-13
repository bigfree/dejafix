use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "djf",
    about = "Project memory for humans and AI tools.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Create .dejafix/ in the current repo.")]
    Init(InitArgs),
    #[command(about = "Print AI instructions for this project.")]
    Instructions,
    #[command(
        about = "Print the project plan (intent file) — or append an idea with `djf plan \"...\"`."
    )]
    Plan {
        #[arg(help = "Optional: append this as a bullet under Ideas. Omit to print plan.md.")]
        add: Option<String>,
    },
    #[command(about = "Print the project map.")]
    Map,
    #[command(
        about = "Register projects so one MCP server can serve them all.",
        arg_required_else_help = true
    )]
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    #[command(about = "Check your setup: unregistered projects, stale entries, pinned configs.")]
    Doctor {
        #[arg(long, help = "Apply the fixes, not just report them.")]
        fix: bool,
        #[arg(
            short,
            long,
            default_value_t = 4,
            allow_negative_numbers = true,
            help = "How deep to scan for projects."
        )]
        depth: i64,
        #[arg(short, long, help = "Scan here instead of the usual places.")]
        path: Vec<String>,
    },
    #[command(about = "Start a new issue.")]
    Log {
        text: String,
        #[arg(long, help = "Location (e.g. file:line, class.method)")]
        at: Option<String>,
    },
    #[command(about = "Record an attempt on the current issue.")]
    Attempt {
        text: String,
        #[arg(long, help = "Mark the attempt as worked.")]
        worked: bool,
        #[arg(long, help = "Mark the attempt as failed.")]
        failed: bool,
        #[arg(long, help = "Mark the attempt as partial.")]
        partial: bool,
        #[arg(long, help = "Location (e.g. file:line, class.method)")]
        at: Option<String>,
        #[arg(long, help = "Attach to a specific issue ID (e.g. 0042).")]
        issue: Option<String>,
        #[arg(
            long = "auto-issue",
            help = "If no active issue exists, auto-create one from this attempt's text."
        )]
        auto_issue: bool,
    },
    #[command(about = "Record a fix and close the active issue, or a specific issue with --issue.")]
    Fix {
        text: String,
        #[arg(long, help = "Location (e.g. file:line, class.method)")]
        at: Option<String>,
        #[arg(
            long,
            help = "Close a specific issue ID instead of the active issue (e.g. 0042)."
        )]
        issue: Option<String>,
    },
    #[command(about = "Record a decision (optionally retiring a prior one).")]
    Decision {
        text: String,
        #[arg(long, help = "Location (e.g. file:line, class.method)")]
        at: Option<String>,
        #[arg(
            long,
            help = "Event id (or unique prefix) of a prior decision this one retires. The old event stays in the log, tagged (superseded); only the new one appears in summary.md."
        )]
        supersedes: Option<String>,
    },
    #[command(about = "Record a free-form note.")]
    Note {
        text: String,
        #[arg(long, help = "Location (e.g. file:line, class.method)")]
        at: Option<String>,
    },
    #[command(about = "Print the current summary.md.")]
    Show,
    #[command(about = "Substring (default) or regex search across events.")]
    Search {
        query: String,
        #[arg(
            short,
            long,
            help = "Treat query as a case-insensitive Python regex (enables OR-patterns)."
        )]
        regex: bool,
        #[arg(
            long = "failed-only",
            help = "Only show failed attempts — the project's catalogue of dead ends."
        )]
        failed_only: bool,
    },
    #[command(about = "Rebuild summary.md from events.jsonl.")]
    Regenerate,
    #[command(about = "Manage dejafix git hooks (install/uninstall).")]
    Hooks {
        #[arg(default_value = "install", help = "Action: install or uninstall.")]
        action: String,
    },
    #[command(
        name = "_auto-capture",
        hide = true,
        about = "Internal: called by git hooks to auto-capture events."
    )]
    AutoCapture {
        #[arg(default_value = "commit", help = "Trigger type: commit, merge.")]
        trigger: String,
    },
    #[command(about = "Check staged changes (or named files) against project memory.")]
    Precheck(PrecheckArgs),
    #[command(
        about = "One-screen session-start briefing: warnings, stale memories, open issues, recent decisions, stack gotchas, and the prevention score."
    )]
    Brief,
    #[command(name = "global", about = "Manage cross-project global memory.")]
    Global(GlobalArgs),
}

#[derive(Debug, Args)]
pub struct GlobalArgs {
    #[arg(
        default_value = "status",
        help = "Action: status, list, add, add-gotcha, remove, export, import, prune, detect."
    )]
    pub action: String,
    #[arg(help = "Pattern text, gotcha text, entry ID, or import file path.")]
    pub text: Option<String>,
    #[arg(long, help = "Filter or tag entries.")]
    pub tag: Option<String>,
    #[arg(long, help = "Filter by or specify library.")]
    pub library: Option<String>,
    #[arg(
        long = "older-than",
        default_value_t = 365,
        allow_negative_numbers = true,
        help = "Days threshold for prune."
    )]
    pub older_than: i64,
    #[arg(long, help = "Confidence filter for prune.")]
    pub confidence: Option<String>,
    #[arg(
        short = 'f',
        long = "format",
        default_value = "text",
        help = "Output format for list/detect: text, json."
    )]
    pub format: String,
}

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one field per command-line flag, as clap derives them"
)]
pub struct InitArgs {
    #[arg(long = "no-hooks", help = "Skip git hook installation.")]
    pub no_hooks: bool,
    #[arg(long = "no-global", help = "Skip global memory inheritance.")]
    pub no_global: bool,
    #[arg(long = "no-watch", hide = true)]
    pub no_watch: bool,
    #[arg(long = "no-backfill", hide = true)]
    pub no_backfill: bool,
    #[arg(
        long = "no-claude-md",
        help = "Skip writing/updating the CLAUDE.md bridge block."
    )]
    pub no_claude_md: bool,
    #[arg(
        long = "no-stack-detect",
        help = "Skip auto-populating PROJECT_MAP.md from detected stack."
    )]
    pub no_stack_detect: bool,
    #[arg(
        long = "no-mcp-config",
        help = "Skip printing the MCP client config block at the end."
    )]
    pub no_mcp_config: bool,
    #[arg(
        long = "mcp-config-single",
        help = "Print the pinned single-project MCP config instead of the shared one."
    )]
    pub mcp_config_single: bool,
    #[arg(long = "no-structure", hide = true)]
    pub no_structure: bool,
    #[arg(
        long = "global-tags",
        help = "Only inherit matching tags (comma-separated)."
    )]
    pub global_tags: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ProjectAction {
    #[command(about = "Add a project to the registry.")]
    Register {
        #[arg(help = "Project folder (default: here).")]
        path: Option<String>,
        #[arg(short, long, help = "Short name to call it by.")]
        alias: Option<String>,
    },
    #[command(about = "Find projects that already have memory and add them to the registry.")]
    Scan {
        #[arg(help = "Where to look. Several are allowed (default: here).")]
        paths: Vec<String>,
        #[arg(
            short,
            long,
            default_value_t = 4,
            allow_negative_numbers = true,
            help = "How deep to walk."
        )]
        depth: i64,
        #[arg(
            short = 'n',
            long = "dry-run",
            help = "Show what would be registered, change nothing."
        )]
        dry_run: bool,
    },
    #[command(about = "Show every registered project.")]
    List,
    #[command(about = "Set (or clear) the active project used when a call names none.")]
    Use {
        #[arg(help = "id, alias or path. Omit to clear the active project.")]
        identifier: Option<String>,
    },
    #[command(about = "Forget a project. Its repo and memory are left untouched.")]
    Remove { identifier: String },
    #[command(about = "Give a project a short name.")]
    Alias { identifier: String, alias: String },
    #[command(about = "Tag a project (or remove a tag with --remove).")]
    Tag {
        identifier: String,
        tag: String,
        #[arg(short, long, help = "Remove the tag.")]
        remove: bool,
    },
}

#[derive(Debug, Args)]
pub struct PrecheckArgs {
    #[arg(
        help = "Specific files to check (e.g. `djf precheck payment.py auth.py`). Default: staged files."
    )]
    pub files: Vec<String>,
    #[arg(
        short,
        long,
        default_value = "warn",
        help = "Strictness: info, warn, block."
    )]
    pub level: String,
    #[arg(long, help = "Check working tree instead of staged.")]
    pub working: bool,
    #[arg(short, long, help = "Only output if warnings present.")]
    pub quiet: bool,
    #[arg(
        long,
        help = "Silence precheck warnings for a duration (30m, 2h, 1d). The snooze is logged to memory, so the silence is audited."
    )]
    pub snooze: Option<String>,
    #[arg(long, help = "Re-enable warnings before the snooze expires.")]
    pub unsnooze: bool,
}
