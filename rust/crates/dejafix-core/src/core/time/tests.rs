use std::borrow::Cow;

use jiff::tz::TimeZone;
use proptest::prelude::*;

use super::normalize_timestamp;

fn denver() -> TimeZone {
    TimeZone::get("America/Denver").unwrap()
}

fn normalized(input: &str, zone: &TimeZone) -> Option<String> {
    normalize_timestamp(Cow::Borrowed(input), zone)
        .ok()
        .map(|stamp| stamp.as_str().to_owned())
}

#[test]
fn normalizes_what_python_normalizes() {
    let cases = [
        ("2026-05-12T21:07:46Z", "2026-05-12T21:07:46Z"),
        ("2026-05-12T21:07:46+02:00", "2026-05-12T19:07:46Z"),
        ("2026-05-12T21:07:46.123456789Z", "2026-05-12T21:07:46Z"),
        ("2026-05-12T21:07:46.1Z", "2026-05-12T21:07:46Z"),
        ("2026-05-12T21:07:46,5Z", "2026-05-12T21:07:46Z"),
        ("2026-05-12 21:07:46 -0600", "2026-05-13T03:07:46Z"),
        ("2026-05-12 21:07:46", "2026-05-13T03:07:46Z"),
        ("2026-05-12", "2026-05-12T06:00:00Z"),
        ("2026-05-12x21:07", "2026-05-13T03:07:00Z"),
        ("2026-05-12é21:07", "2026-05-13T03:07:00Z"),
        ("20260512T210746Z", "2026-05-12T21:07:46Z"),
        ("2026-W20-2", "2026-05-12T06:00:00Z"),
        ("2026W201", "2026-05-11T06:00:00Z"),
        ("2026-W53", "2026-12-28T07:00:00Z"),
        ("2026-05-12T21Z", "2026-05-12T21:00:00Z"),
        ("2026-5-1 1:2:3 +0000", "2026-05-01T01:02:03Z"),
        ("2026-05-12  21:07:46 -0600", "2026-05-13T03:07:46Z"),
        ("2026-05-12\t21:07:46\t+0000", "2026-05-12T21:07:46Z"),
        ("2026-05-12 21:07:46+0000", "2026-05-12T21:07:46Z"),
        ("2026-05-12T21:07:46+0200", "2026-05-12T19:07:46Z"),
        ("2026-05-12T21:07:46+02", "2026-05-12T19:07:46Z"),
        ("2026-05-12T21:07:46+02:00:30", "2026-05-12T19:07:16Z"),
        (
            "2026-05-12T21:07:46+23:59:59.999999",
            "2026-05-11T21:07:46Z",
        ),
        ("2026-05-12 01:07:46 -02:00:30.5", "2026-05-12T03:08:16Z"),
        ("2026-03-08T02:30:00", "2026-03-08T09:30:00Z"),
        ("2026-11-01T01:30:00", "2026-11-01T07:30:00Z"),
        ("2026-05-12T24:00:00", "2026-05-13T06:00:00Z"),
        ("2024-02-29T24:00", "2024-03-01T07:00:00Z"),
        ("1969-12-31T23:59:59.5", "1970-01-01T06:59:59Z"),
        ("1969-12-31T23:59:59.5Z", "1969-12-31T23:59:59Z"),
        ("0999-05-12T21:07:46Z", "0999-05-12T21:07:46Z"),
        ("0001-01-02T00:00:00", "0001-01-02T06:59:56Z"),
        ("9999-12-30T23:59:59", "9999-12-31T06:59:59Z"),
        ("9999-12-31T23:59:59Z", "9999-12-31T23:59:59Z"),
        ("2026-05-12T21:07:46z", "2026-05-12T21:07:46z"),
        ("2026-05-12T21:07:46+24:00", "2026-05-12T21:07:46+24:00"),
        ("2026-05-12 21:07:60 +0000", "2026-05-12 21:07:60 +0000"),
        (
            "2026-05-12 21:07:46 +02:0030",
            "2026-05-12 21:07:46 +02:0030",
        ),
        ("2026-05-12 21:07:46 +99:00", "2026-05-12 21:07:46 +99:00"),
        ("2026-05-12 21:07:46 z", "2026-05-12 21:07:46 z"),
        (
            "2026-05-12 21:07:46 +0200:30",
            "2026-05-12 21:07:46 +0200:30",
        ),
        ("2026-05-12T21:07:46 ", "2026-05-12T21:07:46 "),
        (" 2026-05-12T21:07:46Z", " 2026-05-12T21:07:46Z"),
        ("2026-02-29", "2026-02-29"),
        ("2026-05-12T21:07:46-0", "2026-05-12T21:07:46-0"),
        ("nonsense", "nonsense"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            normalized(input, &denver()).as_deref(),
            Some(expected),
            "{input}"
        );
    }
}

#[test]
fn a_canonical_timestamp_is_not_copied() {
    let stamp = normalize_timestamp(Cow::Borrowed("2026-05-12T21:07:46Z"), &TimeZone::UTC).unwrap();
    assert!(matches!(stamp.text, Cow::Borrowed(_)));
    assert_eq!(
        stamp.utc().unwrap(),
        jiff::civil::date(2026, 5, 12).at(21, 7, 46, 0)
    );
}

#[test]
fn a_utc_year_outside_python_datetime_is_an_overflow() {
    for input in [
        "0001-01-01T00:30:00+01:00",
        "9999-12-31T23:59:59-01:00",
        "0001-01-01T00:00:00",
        "9999-12-31T00:00:00",
    ] {
        assert!(
            normalize_timestamp(Cow::Borrowed(input), &denver()).is_err(),
            "{input}"
        );
    }
    assert_eq!(
        normalized("0001-01-01T00:00:00Z", &denver()).as_deref(),
        Some("0001-01-01T00:00:00Z")
    );
}

#[test]
fn naive_times_use_the_given_zone() {
    let tokyo = TimeZone::get("Asia/Tokyo").unwrap();
    assert_eq!(
        normalized("0001-01-02T00:00:00", &tokyo).as_deref(),
        Some("0001-01-01T14:41:01Z")
    );
    assert_eq!(
        normalized("2026-05-12 21:07:46", &TimeZone::UTC).as_deref(),
        Some("2026-05-12T21:07:46Z")
    );
}

proptest! {
    #[test]
    fn never_panics(input in "\\PC{0,40}") {
        let _ = normalize_timestamp(Cow::Owned(input), &denver());
    }

    #[test]
    fn shaped_input_never_panics(input in "[0-9]{4}[-W]?[0-9]{1,3}-?[0-9]{0,2}[T :é]?[0-9:.,]{0,12}[-+Z]?[0-9:.]{0,12}") {
        let _ = normalize_timestamp(Cow::Owned(input), &denver());
    }

    #[test]
    fn canonical_output_is_a_fixed_point(
        year in 1_i16..=9999, month in 1_i8..=12, day in 1_i8..=28,
        hour in 0_i8..24, minute in 0_i8..60, second in 0_i8..60,
    ) {
        let civil = jiff::civil::date(year, month, day).at(hour, minute, second, 0);
        let text = civil.strftime("%Y-%m-%dT%H:%M:%SZ").to_string();
        let once = normalize_timestamp(Cow::Owned(text.clone()), &denver()).unwrap();
        prop_assert_eq!(once.as_str(), text.as_str());
    }
}
