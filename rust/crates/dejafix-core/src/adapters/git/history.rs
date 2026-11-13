use std::borrow::Cow;
use std::path::Path;

use jiff::tz::TimeZone;

use super::run::git;
use super::types::GitHistory;
use crate::core::stale::FileHistory;
use crate::core::text::python_strip;
use crate::core::time::normalize_timestamp;

impl FileHistory for GitHistory {
    fn exists(&self, relative: &str) -> bool {
        self.root.join(relative).exists()
    }

    fn commit_times(&self, relative: &str, since: Option<&str>) -> Option<Vec<String>> {
        let mut args = vec![Cow::Borrowed("log"), Cow::Borrowed("--format=%cI")];
        if let Some(since) = since {
            args.push(Cow::Owned(format!("--since={since}")));
        }
        args.push(Cow::Borrowed("--"));
        args.push(Cow::Borrowed(relative));
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        let output = git(&self.root, &args)?;
        let text = String::from_utf8_lossy(&output);
        Some(
            text.lines()
                .map(python_strip)
                .filter(|line| !line.is_empty())
                .map(|line| {
                    normalize_timestamp(Cow::Borrowed(line), &TimeZone::UTC)
                        .map_or_else(|_| line.to_owned(), |time| time.as_str().to_owned())
                })
                .collect(),
        )
    }
}

#[must_use]
pub fn recent_changes(root: &Path, relative: &str, days: u32) -> Option<usize> {
    let since = format!("--since={days}.days.ago");
    let output = git(root, &["log", &since, "--oneline", "--", relative])?;
    Some(
        String::from_utf8_lossy(&output)
            .lines()
            .filter(|line| !python_strip(line).is_empty())
            .count(),
    )
}
