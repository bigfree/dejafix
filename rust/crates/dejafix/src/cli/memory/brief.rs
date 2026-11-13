use std::io::{self, Write};

use dejafix_core::{Settings, brief};

use crate::cli::types::Outcome;
use crate::config::Config;

pub fn run(config: &Config, settings: &Settings, out: &mut impl Write) -> io::Result<Outcome> {
    match brief(&config.cwd, settings, config.color) {
        Ok(text) => {
            out.write_all(text.as_bytes())?;
            Ok(Outcome::Done)
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}
