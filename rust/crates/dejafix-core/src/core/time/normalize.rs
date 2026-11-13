use std::borrow::Cow;

use jiff::SignedDuration;
use jiff::civil::DateTime;
use jiff::tz::{AmbiguousOffset, TimeZone};

use super::types::{Clock, Parsed, Timestamp, TimestampOverflow};
use super::{iso, strptime};

const FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

pub fn normalize_timestamp<'a>(
    raw: Cow<'a, str>,
    zone: &TimeZone,
) -> Result<Timestamp<'a>, TimestampOverflow> {
    if let Some(utc) = canonical(&raw) {
        return Ok(Timestamp {
            text: raw,
            utc: Some(utc),
        });
    }
    let parsed = if raw.contains('Z') {
        iso::parse(&raw.replace('Z', "+00:00"))
    } else {
        iso::parse(&raw)
    }
    .or_else(|| strptime::parse(&raw));
    let Some(parsed) = parsed else {
        return Ok(Timestamp {
            text: raw,
            utc: None,
        });
    };
    let utc = to_utc(parsed, zone).ok_or(TimestampOverflow)?;
    let canonical = utc.strftime(FORMAT).to_string();
    let text = if canonical == *raw {
        raw
    } else {
        Cow::Owned(canonical)
    };
    Ok(Timestamp {
        text,
        utc: Some(utc),
    })
}

fn canonical(text: &str) -> Option<DateTime> {
    let bytes: &[u8; 20] = text.as_bytes().try_into().ok()?;
    let shape = bytes.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 => *byte == b'-',
        10 => *byte == b'T',
        13 | 16 => *byte == b':',
        19 => *byte == b'Z',
        _ => byte.is_ascii_digit(),
    });
    if !shape {
        return None;
    }
    let field = |range: std::ops::Range<usize>| text.get(range)?.parse::<i16>().ok();
    let year = field(0..4).filter(|year| *year >= 1)?;
    let narrow = |range| field(range).and_then(|value| i8::try_from(value).ok());
    let date = jiff::civil::Date::new(year, narrow(5..7)?, narrow(8..10)?).ok()?;
    let time = jiff::civil::Time::new(narrow(11..13)?, narrow(14..16)?, narrow(17..19)?, 0).ok()?;
    Some(date.to_datetime(time))
}

fn to_utc(parsed: Parsed, zone: &TimeZone) -> Option<DateTime> {
    let offset_micros = parsed
        .offset_micros
        .or_else(|| local_offset_micros(parsed.civil, zone))?;
    let utc = parsed
        .civil
        .checked_sub(SignedDuration::from_micros(offset_micros))
        .ok()?
        .with()
        .subsec_nanosecond(0)
        .build()
        .ok()?;
    (1..=9999).contains(&utc.year()).then_some(utc)
}

fn local_offset_micros(civil: DateTime, zone: &TimeZone) -> Option<i64> {
    let day = SignedDuration::from_hours(24);
    let earlier = civil.checked_sub(day).ok()?;
    let later = civil.checked_add(day).ok()?;
    if earlier.year() < 1 || later.year() > 9999 {
        return None;
    }
    let offset = match zone.to_ambiguous_timestamp(civil).offset() {
        AmbiguousOffset::Unambiguous { offset }
        | AmbiguousOffset::Gap { before: offset, .. }
        | AmbiguousOffset::Fold { before: offset, .. } => offset,
    };
    Some(i64::from(offset.seconds()) * 1_000_000)
}

impl Timestamp<'_> {
    #[must_use]
    pub fn into_owned(self) -> Timestamp<'static> {
        Timestamp {
            text: Cow::Owned(self.text.into_owned()),
            utc: self.utc,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn utc(&self) -> Option<DateTime> {
        self.utc
    }

    #[must_use]
    pub fn now(clock: &Clock) -> Timestamp<'static> {
        let utc = TimeZone::UTC.to_datetime(clock.now);
        let utc = utc.with().subsec_nanosecond(0).build().unwrap_or(utc);
        Timestamp {
            text: Cow::Owned(utc.strftime(FORMAT).to_string()),
            utc: Some(utc),
        }
    }
}

#[must_use]
pub fn iso_micros(clock: &Clock) -> String {
    let utc = TimeZone::UTC.to_datetime(clock.now);
    let micros = utc.subsec_nanosecond() / 1_000;
    let seconds = utc.strftime(FORMAT).to_string();
    if micros == 0 {
        return seconds;
    }
    let stem = seconds.strip_suffix('Z').unwrap_or(&seconds);
    format!("{stem}.{micros:06}Z")
}
