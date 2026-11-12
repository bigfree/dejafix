use std::io::{self, Write};

use dejafix_core::{Settings, upgrade_notice};

use super::output::secho;
use super::types::Style;

pub fn print(
    settings: &Settings,
    version: &str,
    out: &mut impl Write,
    color: bool,
) -> io::Result<()> {
    let Some(notice) = upgrade_notice(settings, version) else {
        return Ok(());
    };
    secho(
        out,
        &format!("\ndejafix upgraded to {}.", notice.version),
        Style::Cyan,
        color,
    )?;
    writeln!(
        out,
        "  Run `djf init` in each project to refresh its instructions and hooks."
    )?;
    writeln!(out, "  Check your setup with:  djf doctor\n")
}
