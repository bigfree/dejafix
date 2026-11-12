use std::fmt::Write;

use super::types::{Severity, Styles, Warning};

const RULE: &str = "─";
const WIDTH: usize = 60;

const COLOURED: Styles = Styles {
    bold: "\x1b[1m",
    dim: "\x1b[2m",
    yellow: "\x1b[33m",
    red: "\x1b[31m",
    cyan: "\x1b[36m",
    reset: "\x1b[0m",
};

const PLAIN: Styles = Styles {
    bold: "",
    dim: "",
    yellow: "",
    red: "",
    cyan: "",
    reset: "",
};

#[must_use]
pub fn render_warnings(warnings: &[Warning], level: &str, color: bool) -> Option<String> {
    let visible: Vec<&Warning> = warnings
        .iter()
        .filter(|warning| warning.severity != Severity::Info || level == "info")
        .collect();
    if visible.is_empty() {
        return None;
    }
    let Styles {
        bold,
        dim,
        yellow,
        red,
        cyan,
        reset,
    } = if color { COLOURED } else { PLAIN };
    let rule = RULE.repeat(WIDTH);
    let mut out = String::new();
    let _ = write!(
        out,
        "\n{bold}dejafix: Pre-Commit Check{reset}\n{dim}{rule}{reset}\n\n"
    );
    let mut files: Vec<&str> = Vec::new();
    for warning in &visible {
        if !files.contains(&warning.file.as_str()) {
            files.push(&warning.file);
        }
    }
    for file in files {
        let _ = writeln!(out, "  {bold}{file}{reset}");
        for warning in visible.iter().filter(|warning| warning.file == file) {
            let icon = match warning.severity {
                Severity::Block => format!("{red}BLOCK{reset}"),
                Severity::Warn => format!("{yellow}WARN{reset}"),
                Severity::Info => format!("{cyan}INFO{reset}"),
            };
            let _ = writeln!(out, "    {icon}  {}", warning.title);
            for detail in &warning.details {
                let _ = writeln!(out, "           {dim}{detail}{reset}");
            }
        }
        out.push('\n');
    }
    let _ = writeln!(out, "{dim}{rule}{reset}");
    let blocking = visible
        .iter()
        .filter(|warning| warning.severity == Severity::Block)
        .count();
    let warning = visible
        .iter()
        .filter(|warning| warning.severity == Severity::Warn)
        .count();
    if blocking > 0 && level == "block" {
        let _ = writeln!(
            out,
            "{red}Blocked: {blocking} critical warning(s).{reset}\n  Bypass once: git commit --no-verify"
        );
    } else if warning > 0 || blocking > 0 {
        let _ = writeln!(
            out,
            "{dim}{} warning(s). Review before committing.{reset}",
            warning + blocking
        );
    }
    out.push('\n');
    Some(out)
}
