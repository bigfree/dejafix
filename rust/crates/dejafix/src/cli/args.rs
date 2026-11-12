use clap::{Parser, Subcommand};

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
}
