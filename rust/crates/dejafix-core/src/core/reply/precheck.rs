use std::collections::HashMap;
use std::fmt::Write as _;

use crate::core::path::{PathContext, PathResolver, is_memory_path, project_relative};
use crate::core::precheck::{Severity, Warning};
use crate::core::text::python_strip;

fn severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "INFO",
        Severity::Warn => "WARN",
        Severity::Block => "BLOCK",
    }
}

fn block(
    path: &str,
    warnings: &[&Warning],
    context: &PathContext,
    resolver: &impl PathResolver,
) -> String {
    if is_memory_path(path, context, resolver) {
        return format!(
            "{path}: this is a dejafix memory file. It is regenerated from the event log, so do not edit it; record the change with add_note / add_decision / log_issue / record_fix instead."
        );
    }
    if warnings.is_empty() {
        return format!("{path}: no warnings. Safe to modify.");
    }
    let mut text = format!("dejafix precheck: {path}");
    for warning in warnings {
        let _ = write!(
            text,
            "\n  [{}] {}",
            severity(warning.severity),
            warning.title
        );
        for detail in &warning.details {
            let _ = write!(text, "\n    - {detail}");
        }
    }
    text
}

#[must_use]
pub fn precheck_report(
    paths: &[String],
    warnings: &[Warning],
    context: &PathContext,
    resolver: &impl PathResolver,
) -> String {
    let mut by_file: HashMap<&str, Vec<&Warning>> = HashMap::new();
    for warning in warnings {
        by_file
            .entry(warning.file.as_str())
            .or_default()
            .push(warning);
    }
    paths
        .iter()
        .map(|path| {
            let key = project_relative(python_strip(path), context, resolver);
            let found = by_file.get(key.as_str()).map_or(&[][..], Vec::as_slice);
            block(path, found, context, resolver)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
