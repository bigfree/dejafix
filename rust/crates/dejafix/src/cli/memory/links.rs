use std::io::{self, Write};

use dejafix_core::{Settings, links_listing};

use crate::cli::output::printable_text;
use crate::cli::types::Outcome;
use crate::config::Config;

pub fn run(config: &Config, settings: &Settings, out: &mut impl Write) -> io::Result<Outcome> {
    match links_listing(&config.cwd, settings) {
        Ok(text) => {
            for line in text.lines() {
                writeln!(out, "{}", printable_text(line))?;
            }
            Ok(Outcome::Done)
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}
