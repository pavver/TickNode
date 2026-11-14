use super::*;
use ticknode_shared::TimeDelta;

#[test]
fn bare_numbers_use_selected_units_and_explicit_text_overrides_them() {
    for (unit, input, expected) in [
        (Unit::Ticks, "10", DataValue::Ticks(10)),
        (
            Unit::Milliseconds,
            "500",
            DataValue::Duration(TimeDelta::try_milliseconds(500).unwrap()),
        ),
        (
            Unit::Milliseconds,
            "-500",
            DataValue::Duration(TimeDelta::try_milliseconds(-500).unwrap()),
        ),
        (
            Unit::Seconds,
            "1,5",
            DataValue::Duration(TimeDelta::try_milliseconds(1500).unwrap()),
        ),
        (
            Unit::Seconds,
            "-1",
            DataValue::Duration(TimeDelta::try_seconds(-1).unwrap()),
        ),
        (
            Unit::Minutes,
            "2",
            DataValue::Duration(TimeDelta::try_seconds(120).unwrap()),
        ),
        (
            Unit::Minutes,
            "-1,5",
            DataValue::Duration(TimeDelta::try_seconds(-90).unwrap()),
        ),
        (
            Unit::Hours,
            "2",
            DataValue::Duration(TimeDelta::try_seconds(7200).unwrap()),
        ),
        (
            Unit::Ticks,
            "1h 30m",
            DataValue::Duration(TimeDelta::try_seconds(5400).unwrap()),
        ),
        (
            Unit::Ticks,
            "-1h 30m",
            DataValue::Duration(TimeDelta::try_seconds(-5400).unwrap()),
        ),
        (Unit::Hours, "10t", DataValue::Ticks(10)),
        (Unit::Seconds, "10 тактів", DataValue::Ticks(10)),
        (
            Unit::Hours,
            "1.5ms",
            DataValue::Duration(TimeDelta::microseconds(1500)),
        ),
    ] {
        assert_eq!(parse(input, unit), Ok(Some(expected)));
    }
    assert_eq!(parse("  ", Unit::Ticks), Ok(None));
    for input in ["-1", "1.5", "NaN", "inf", "18446744073709551616", "oops"] {
        let error = parse(input, Unit::Ticks).unwrap_err();
        assert_eq!(error.key, "parse-error-duration");
        assert_eq!(error.args, [("value".into(), input.into())]);
    }
    assert!(parse("1e300", Unit::Hours).is_err());
}

#[test]
fn whole_values_choose_units_and_keep_user_selection_when_possible() {
    for (value, number, unit) in [
        (DataValue::Ticks(10), "10", Unit::Ticks),
        (
            DataValue::Duration(TimeDelta::try_seconds(7200).unwrap()),
            "2",
            Unit::Hours,
        ),
        (
            DataValue::Duration(TimeDelta::try_seconds(-7200).unwrap()),
            "-2",
            Unit::Hours,
        ),
        (
            DataValue::Duration(TimeDelta::try_seconds(5400).unwrap()),
            "90",
            Unit::Minutes,
        ),
        (
            DataValue::Duration(TimeDelta::try_seconds(7).unwrap()),
            "7",
            Unit::Seconds,
        ),
        (
            DataValue::Duration(TimeDelta::try_seconds(-7).unwrap()),
            "-7",
            Unit::Seconds,
        ),
        (
            DataValue::Duration(TimeDelta::try_milliseconds(1500).unwrap()),
            "1500",
            Unit::Milliseconds,
        ),
    ] {
        assert_eq!(presentation(Some(&value), None), (number.into(), unit));
        assert_eq!(parse(number, unit), Ok(Some(value)));
    }
    let minute = DataValue::Duration(TimeDelta::try_seconds(60).unwrap());
    assert_eq!(
        presentation(Some(&minute), Some(Unit::Seconds)),
        ("60".into(), Unit::Seconds)
    );
    let neg_minute = DataValue::Duration(TimeDelta::try_seconds(-60).unwrap());
    assert_eq!(
        presentation(Some(&neg_minute), Some(Unit::Seconds)),
        ("-60".into(), Unit::Seconds)
    );
    assert_eq!(
        presentation(None, Some(Unit::Ticks)),
        (String::new(), Unit::Ticks)
    );
    let max = DataValue::Ticks(u64::MAX);
    let (number, unit) = presentation(Some(&max), None);
    assert_eq!(parse(&number, unit), Ok(Some(max)));
    let large = DataValue::Duration(TimeDelta::try_seconds(86400 * 365).unwrap());
    let (number, unit) = presentation(Some(&large), None);
    assert_eq!(parse(&number, unit), Ok(Some(large)));
}

#[test]
fn fractional_values_remain_lossless_text() {
    for duration in [
        TimeDelta::nanoseconds(1),
        TimeDelta::microseconds(1500),
        TimeDelta::new(61, 123_456_789).unwrap(),
        TimeDelta::new(-62, 876_543_211).unwrap(),
    ] {
        let value = DataValue::Duration(duration);
        let (input, unit) = presentation(Some(&value), Some(Unit::Seconds));
        assert!(input.ends_with('s'));
        assert_eq!(parse(&input, unit), Ok(Some(value)));
    }
}

#[test]
fn property_durations_offer_only_wall_units_and_reject_explicit_ticks() {
    assert_eq!(
        Unit::available(true).collect::<Vec<_>>(),
        [
            Unit::Milliseconds,
            Unit::Seconds,
            Unit::Minutes,
            Unit::Hours
        ]
    );
    assert_eq!(Unit::available(false).collect::<Vec<_>>(), Unit::ALL);
    assert_eq!(
        presentation(None, Some(Unit::Minutes)),
        (String::new(), Unit::Minutes)
    );
    assert_eq!(
        parse_property("30", Unit::Minutes),
        Ok(Some(DataValue::Duration(
            TimeDelta::try_seconds(1800).unwrap()
        )))
    );
    assert_eq!(
        parse_property("00:30:00", Unit::Minutes),
        Ok(Some(DataValue::Duration(
            TimeDelta::try_seconds(1800).unwrap()
        )))
    );
    assert_eq!(parse_property("", Unit::Minutes), Ok(None));
    for invalid in ["10t", "10 ticks", "10 тактів", "-30", "garbage"] {
        assert_eq!(
            parse_property(invalid, Unit::Minutes).unwrap_err().key,
            "error-property-duration"
        );
    }
}
