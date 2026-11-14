use super::types::{Command, ValidationError};
use crate::core::text::{LanguageVerdict, language_verdict};

const DOUBTFUL_NOTICE: &str =
    "This does not look like English; if it is not, rewrite it in English.";

fn text(command: &Command) -> &str {
    match command {
        Command::Log { text, .. }
        | Command::Attempt { text, .. }
        | Command::Fix { text, .. }
        | Command::Decision { text, .. }
        | Command::Note { text, .. } => text,
    }
}

pub fn language_notice(command: &Command) -> Result<Option<&'static str>, ValidationError> {
    match language_verdict(text(command)) {
        LanguageVerdict::English => Ok(None),
        LanguageVerdict::Doubtful => Ok(Some(DOUBTFUL_NOTICE)),
        LanguageVerdict::Rejected { reason } => {
            Err(ValidationError::NotEnglish(reason.to_string()))
        }
    }
}
