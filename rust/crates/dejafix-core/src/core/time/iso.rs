use jiff::civil::{Date, DateTime, ISOWeekDate, Time, Weekday};

use super::types::{Bytes, ClockTime, Parsed};

const CORRECTION: [i64; 5] = [100_000, 10_000, 1_000, 100, 10];

impl Bytes<'_> {
    fn at(&self, index: usize) -> u8 {
        self.0.get(index).copied().unwrap_or(0)
    }

    fn digits(&self, start: usize, count: usize) -> Option<i64> {
        (start..start + count).try_fold(0_i64, |value, index| {
            let byte = self.at(index);
            byte.is_ascii_digit()
                .then(|| value * 10 + i64::from(byte - b'0'))
        })
    }
}

pub(super) fn parse(text: &str) -> Option<Parsed> {
    if text.chars().count() < 7 {
        return None;
    }
    let bytes = Bytes(text.as_bytes());
    let length = text.len();
    let separator = separator_location(&bytes, length)?;
    let mut date = parse_date(&bytes, separator)?;
    let mut clock = ClockTime {
        hour: 0,
        minute: 0,
        second: 0,
        micros: 0,
    };
    let mut offset_micros = None;
    if length > separator {
        let width = match bytes.at(separator) {
            byte if byte & 0x80 == 0 => 1,
            byte if byte & 0xf0 == 0xe0 => 3,
            byte if byte & 0xf0 == 0xf0 => 4,
            _ => 2,
        };
        let start = separator + width;
        let rest = Bytes(text.as_bytes().get(start..).unwrap_or_default());
        let remaining = length.saturating_sub(start);
        (clock, offset_micros) = parse_time(&rest, remaining)?;
    }
    if clock.hour == 24 {
        if (clock.minute, clock.second, clock.micros) != (0, 0, 0) {
            return None;
        }
        clock.hour = 0;
        date = date.tomorrow().ok()?;
    }
    Some(Parsed {
        civil: civil(date, clock)?,
        offset_micros,
    })
}

pub(super) fn civil(date: Date, clock: ClockTime) -> Option<DateTime> {
    let time = Time::new(
        i8::try_from(clock.hour).ok()?,
        i8::try_from(clock.minute).ok()?,
        i8::try_from(clock.second).ok()?,
        i32::try_from(clock.micros.checked_mul(1_000)?).ok()?,
    )
    .ok()?;
    Some(date.to_datetime(time))
}

fn separator_location(bytes: &Bytes<'_>, length: usize) -> Option<usize> {
    if length == 7 {
        return Some(7);
    }
    if bytes.at(4) == b'-' {
        if bytes.at(5) != b'W' {
            return Some(10);
        }
        if length > 8 && bytes.at(8) == b'-' {
            if length == 9 {
                return None;
            }
            if length > 10 && bytes.at(10).is_ascii_digit() {
                return Some(8);
            }
            return Some(10);
        }
        return Some(8);
    }
    if bytes.at(4) == b'W' {
        let end = (7..length)
            .find(|&index| !bytes.at(index).is_ascii_digit())
            .unwrap_or(length.max(7));
        if end < 9 {
            return Some(end);
        }
        return Some(if end % 2 == 0 { 7 } else { 8 });
    }
    Some(8)
}

fn parse_date(bytes: &Bytes<'_>, length: usize) -> Option<Date> {
    let year = bytes.digits(0, 4)?;
    let dashed = bytes.at(4) == b'-';
    let mut position = 4 + usize::from(dashed);
    if bytes.at(position) == b'W' {
        position += 1;
        let week = bytes.digits(position, 2)?;
        position += 2;
        let mut day = 1;
        if position < length {
            if dashed {
                if bytes.at(position) != b'-' {
                    return None;
                }
                position += 1;
            }
            day = bytes.digits(position, 1)?;
        }
        if !(1..=9999).contains(&year) || !(1..=7).contains(&day) {
            return None;
        }
        let weekday = Weekday::from_monday_one_offset(i8::try_from(day).ok()?).ok()?;
        return ISOWeekDate::new(i16::try_from(year).ok()?, i8::try_from(week).ok()?, weekday)
            .ok()
            .map(ISOWeekDate::date);
    }
    let month = bytes.digits(position, 2)?;
    position += 2;
    if dashed {
        if bytes.at(position) != b'-' {
            return None;
        }
        position += 1;
    }
    let day = bytes.digits(position, 2)?;
    if year < 1 {
        return None;
    }
    Date::new(
        i16::try_from(year).ok()?,
        i8::try_from(month).ok()?,
        i8::try_from(day).ok()?,
    )
    .ok()
}

fn hh_mm_ss_ff(bytes: &Bytes<'_>, start: usize, end: usize) -> Option<(ClockTime, bool)> {
    let mut parts = [0_i64; 3];
    let mut position = start;
    let mut separated = true;
    for (index, part) in parts.iter_mut().enumerate() {
        *part = bytes.digits(position, 2)?;
        position += 2;
        let next = bytes.at(position);
        position += 1;
        if index == 0 {
            separated = next == b':';
        }
        if next == b'.' || next == b',' {
            if index < 2 || position >= end {
                return None;
            }
            break;
        } else if position >= end {
            let [hour, minute, second] = parts;
            return Some((
                ClockTime {
                    hour,
                    minute,
                    second,
                    micros: 0,
                },
                next != 0,
            ));
        } else if separated && next == b':' {
            if index == 2 {
                return None;
            }
        } else if !separated {
            position -= 1;
        } else {
            return None;
        }
    }
    let [hour, minute, second] = parts;
    let remaining = end.saturating_sub(position);
    let count = remaining.min(6);
    let mut micros = bytes.digits(position, count)?;
    if let Some(scale) = count.checked_sub(1).and_then(|index| CORRECTION.get(index)) {
        micros *= scale;
    }
    position += count;
    while bytes.at(position).is_ascii_digit() {
        position += 1;
    }
    Some((
        ClockTime {
            hour,
            minute,
            second,
            micros,
        },
        bytes.at(position) != 0,
    ))
}

fn parse_time(bytes: &Bytes<'_>, length: usize) -> Option<(ClockTime, Option<i64>)> {
    let zone_at = if length == 0 {
        1
    } else {
        (0..length)
            .find(|&index| matches!(bytes.at(index), b'Z' | b'+' | b'-'))
            .unwrap_or(length)
    };
    let (clock, trailing) = hh_mm_ss_ff(bytes, 0, zone_at)?;
    if zone_at == length {
        return (!trailing).then_some((clock, None));
    }
    if bytes.at(zone_at) == b'Z' {
        return (bytes.at(zone_at + 1) == 0).then_some((clock, Some(0)));
    }
    let negative = bytes.at(zone_at) == b'-';
    let (offset, trailing) = hh_mm_ss_ff(bytes, zone_at + 1, length)?;
    if offset.hour > 23
        || offset.minute > 59
        || offset.second > 59
        || offset.micros > 999_999
        || trailing
    {
        return None;
    }
    let magnitude =
        ((offset.hour * 60 + offset.minute) * 60 + offset.second) * 1_000_000 + offset.micros;
    Some((clock, Some(if negative { -magnitude } else { magnitude })))
}
