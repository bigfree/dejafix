use std::sync::LazyLock;

use jiff::civil::Date;
use regex::{Captures, Regex};

use super::iso::civil;
use super::types::{ClockTime, Parsed};

const SPACE: &str = r"[\s\x1c-\x1f]+";

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static GIT_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\A(?P<Y>[0-9]{{4}})-(?P<m>1[0-2]|0[1-9]|[1-9])-(?P<d>3[0-1]|[1-2][0-9]|0[1-9]|[1-9]| [1-9]){SPACE}(?P<H>2[0-3]|[0-1][0-9]|[0-9]| [0-9]):(?P<M>[0-5][0-9]|[0-9]):(?P<S>6[0-1]|[0-5][0-9]|[0-9]){SPACE}(?P<z>[+-][0-9][0-9]:?[0-5][0-9](?::?[0-5][0-9](?:\.[0-9]{{1,6}})?)?|Z)?"
    ))
    .expect("valid strptime pattern")
});

pub(super) fn parse(text: &str) -> Option<Parsed> {
    let found = GIT_DATE.captures(text)?;
    if found.get(0)?.end() != text.len() {
        return None;
    }
    let date = Date::new(
        i16::try_from(number(&found, "Y")?).ok()?,
        i8::try_from(number(&found, "m")?).ok()?,
        i8::try_from(number(&found, "d")?).ok()?,
    )
    .ok()?;
    let clock = ClockTime {
        hour: number(&found, "H")?,
        minute: number(&found, "M")?,
        second: number(&found, "S")?,
        micros: 0,
    };
    let offset_micros = match found.name("z").map(|zone| zone.as_str()) {
        None => None,
        Some(zone) => Some(offset(zone)?),
    };
    if date.year() < 1 {
        return None;
    }
    Some(Parsed {
        civil: civil(date, clock)?,
        offset_micros,
    })
}

fn number(found: &Captures<'_>, name: &str) -> Option<i64> {
    found.name(name)?.as_str().trim_start().parse().ok()
}

fn offset(zone: &str) -> Option<i64> {
    if zone == "Z" {
        return Some(0);
    }
    let mut compact = zone.to_owned();
    if compact.get(3..4) == Some(":") {
        compact.remove(3);
        if compact.len() > 5 {
            if compact.get(5..6) != Some(":") {
                return None;
            }
            compact.remove(5);
        }
    }
    let hours: i64 = compact.get(1..3)?.parse().ok()?;
    let minutes: i64 = compact.get(3..5)?.parse().ok()?;
    let seconds: i64 = match compact.get(5..7) {
        None | Some("") => 0,
        Some(text) => text.parse().ok()?,
    };
    let fraction = compact.get(8..).unwrap_or_default();
    let micros: i64 = format!("{fraction:0<6}").parse().ok()?;
    let magnitude = ((hours * 60 + minutes) * 60 + seconds) * 1_000_000 + micros;
    if magnitude >= 86_400_000_000 {
        return None;
    }
    Some(if zone.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}
