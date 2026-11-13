use super::types::{McpState, ToolError, WriteCall};
use crate::core::event::Outcome;
use crate::core::time::Clock;
use crate::core::write::{Command, Ids, WriteOutcome, sole_open_issue};
use crate::wiring::record::record_with;
use crate::wiring::types::Project;

fn chosen(issue_id: Option<String>) -> Option<String> {
    issue_id.filter(|issue| !issue.is_empty())
}

fn command(
    call: WriteCall,
    log: &crate::core::log::EventLog<'_>,
) -> Result<Command, crate::core::write::ValidationError> {
    let sole = || -> Result<Option<String>, crate::core::write::ValidationError> {
        Ok(sole_open_issue(log)?.map(|issue| issue.as_str().to_owned()))
    };
    Ok(match call {
        WriteCall::LogIssue { summary, location } => Command::Log {
            text: summary,
            location,
        },
        WriteCall::RecordAttempt {
            summary,
            outcome,
            location,
            issue_id,
        } => Command::Attempt {
            text: summary,
            outcomes: vec![outcome],
            location,
            issue: match chosen(issue_id) {
                Some(issue) => Some(issue),
                None => sole()?,
            },
            auto_issue: true,
        },
        WriteCall::RecordFix {
            summary,
            location,
            issue_id,
        } => Command::Fix {
            text: summary,
            location,
            issue: match chosen(issue_id) {
                Some(issue) => Some(issue),
                None => sole()?,
            },
        },
        WriteCall::AddDecision {
            summary,
            location,
            supersedes,
        } => Command::Decision {
            text: summary,
            location,
            supersedes,
        },
        WriteCall::AddNote { summary, location } => Command::Note {
            text: summary,
            location,
        },
    })
}

impl McpState {
    pub(super) fn write(
        &self,
        call: WriteCall,
        project: Option<&str>,
        clock: &Clock,
        ids: &mut impl Ids,
    ) -> Result<String, ToolError> {
        let target = self.resolution(project, clock)?;
        let opened = Project::at(&target.root, self.project_settings(clock))?;
        let recorded = record_with(&opened, |log| command(call, log), ids)?;
        if let Some(error) = recorded.incomplete {
            return Err(error.into());
        }
        let name = &target.name;
        let last = recorded.events.last().map_or("", String::as_str);
        Ok(match recorded.outcome {
            WriteOutcome::Logged(issue) => format!("Logged issue #{} → {name}", issue.as_str()),
            WriteOutcome::Attempted { outcome, issue } => format!(
                "Recorded {} attempt on #{} → {name}",
                Outcome::as_str(outcome),
                issue.as_str()
            ),
            WriteOutcome::Fixed(issue) => format!("Fixed issue #{} → {name}", issue.as_str()),
            WriteOutcome::Decided { superseded } => {
                let retired = superseded
                    .map(|(id, _)| format!(" (supersedes {})", id.as_str()))
                    .unwrap_or_default();
                format!("Recorded decision {last} → {name}{retired}")
            }
            WriteOutcome::Noted => format!("Recorded note {last} → {name}"),
        })
    }
}
