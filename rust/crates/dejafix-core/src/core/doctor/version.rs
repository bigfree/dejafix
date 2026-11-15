use std::num::ParseIntError;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::types::{DevRelease, PreRelease, Version};

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^\s*v?
        (?P<release>[0-9]+(?:\.[0-9]+)*)
        (?:[-_.]?(?P<pre_l>alpha|beta|preview|pre|rc|a|b|c)[-_.]?(?P<pre_n>[0-9]+)?)?
        (?:-(?P<post_n1>[0-9]+)|[-_.]?(?P<post_l>post|rev|r)[-_.]?(?P<post_n2>[0-9]+)?)?
        (?:[-_.]?(?P<dev_l>dev)[-_.]?(?P<dev_n>[0-9]+)?)?
        \s*$",
    )
    .expect("valid version pattern")
});

fn number(captures: &Captures<'_>, name: &str) -> Result<Option<u64>, ParseIntError> {
    captures
        .name(name)
        .map(|found| found.as_str().parse())
        .transpose()
}

fn phase(label: &str, number: u64) -> PreRelease {
    match label.to_ascii_lowercase().as_str() {
        "a" | "alpha" => PreRelease::Alpha(number),
        "b" | "beta" => PreRelease::Beta(number),
        _ => PreRelease::Candidate(number),
    }
}

pub(crate) fn parse_version(text: &str) -> Option<Version> {
    let captures = PATTERN.captures(text)?;
    let mut release = captures
        .name("release")?
        .as_str()
        .split('.')
        .map(|chunk| chunk.parse().ok())
        .collect::<Option<Vec<u64>>>()?;
    while release.last() == Some(&0) {
        release.pop();
    }
    let pre_number = number(&captures, "pre_n").ok()?.unwrap_or(0);
    let post = match (number(&captures, "post_n1").ok()?, captures.name("post_l")) {
        (Some(value), _) => Some(value),
        (None, Some(_)) => Some(number(&captures, "post_n2").ok()?.unwrap_or(0)),
        (None, None) => None,
    };
    let dev = match captures.name("dev_l") {
        Some(_) => DevRelease::Dev(number(&captures, "dev_n").ok()?.unwrap_or(0)),
        None => DevRelease::Absent,
    };
    let pre = match captures.name("pre_l") {
        Some(label) => phase(label.as_str(), pre_number),
        None if post.is_none() && dev != DevRelease::Absent => PreRelease::DevOnly,
        None => PreRelease::Final,
    };
    Some(Version {
        release,
        pre,
        post,
        dev,
    })
}

#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!(
        (parse_version(candidate), parse_version(current)),
        (Some(candidate), Some(current)) if candidate > current
    )
}
