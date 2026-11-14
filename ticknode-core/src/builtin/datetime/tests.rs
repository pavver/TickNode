use crate::*;
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, properties: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

fn pull(engine: &mut TickEngine, id: NodeId, pin: u32) -> Option<DataValue> {
    engine.pull_data(id, PinId(pin), &mut Vec::new())
}

#[test]
fn now_node_outputs_utc_now_and_local_time_of_day() {
    let mut engine = TickEngine::new();
    let override_dt = DateTime::parse_from_rfc3339("2026-03-15T14:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    engine.host_state.clock_override = Some(override_dt);

    let id = add(&mut engine, "time/now", &[]);
    assert_eq!(
        pull(&mut engine, id, 0),
        Some(DataValue::DateTime(override_dt))
    );

    let expected_local_time = override_dt.with_timezone(&chrono::Local).time();
    assert_eq!(
        pull(&mut engine, id, 1),
        Some(DataValue::Time(expected_local_time))
    );
}

#[test]
fn time_difference_datetime_positive_and_negative() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "time/difference", &[]);

    let t1 = DateTime::parse_from_rfc3339("2026-03-15T14:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let t2 = DateTime::parse_from_rfc3339("2026-03-15T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(t1)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::DateTime(t2)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Duration(TimeDelta::hours(4)))
    );

    // Negative difference
    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(t2)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::DateTime(t1)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Duration(TimeDelta::hours(-4)))
    );
}

#[test]
fn time_difference_time_of_day_plain_difference() {
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "time/difference",
        &[("value_type".into(), "time".into())],
    );

    let a = NaiveTime::from_hms_opt(14, 30, 0).unwrap();
    let b = NaiveTime::from_hms_opt(10, 15, 0).unwrap();

    engine
        .set_input_value(id, PinId(0), Some(DataValue::Time(a)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(b)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Duration(TimeDelta::minutes(255)))
    );

    // Negative plain difference: 10:15 - 14:30 = -255 minutes (no wrap)
    engine
        .set_input_value(id, PinId(0), Some(DataValue::Time(b)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(a)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Duration(TimeDelta::minutes(-255)))
    );
}

#[test]
fn shift_time_add_subtract_and_negative_duration_for_datetime() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "time/shift", &[]);

    let dt = DateTime::parse_from_rfc3339("2026-03-15T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let dur = TimeDelta::hours(2);

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(dt)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Duration(dur)))
        .unwrap();

    let expected_add = DateTime::parse_from_rfc3339("2026-03-15T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::DateTime(expected_add))
    );

    // Subtract
    engine
        .set_node_property(id, "operation", "subtract".into())
        .unwrap();
    let expected_sub = DateTime::parse_from_rfc3339("2026-03-15T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::DateTime(expected_sub))
    );

    // Negative duration with Add: 10:00 + (-2h) = 08:00
    engine
        .set_node_property(id, "operation", "add".into())
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Duration(-dur)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::DateTime(expected_sub))
    );

    // Negative duration with Subtract: 10:00 - (-2h) = 12:00
    engine
        .set_node_property(id, "operation", "subtract".into())
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::DateTime(expected_add))
    );
}

#[test]
fn shift_time_overflow_reports_error_once_and_recovers() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "time/shift", &[]);

    let max_dt = DateTime::<Utc>::MAX_UTC;
    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(max_dt)))
        .unwrap();
    engine
        .set_input_value(
            id,
            PinId(1),
            Some(DataValue::Duration(TimeDelta::days(1000))),
        )
        .unwrap();

    assert_eq!(pull(&mut engine, id, 2), None);
    assert_eq!(engine.host_state.logs.len(), 1);
    assert!(engine.host_state.logs[0].1.contains("overflow"));
    assert_eq!(engine.host_state.errors.len(), 1);
    assert_eq!(
        engine.host_state.errors[0].1.key,
        "error-time-shift-overflow"
    );

    // Second pull while still in overflow: keeps UI error alive without duplicate log
    assert_eq!(pull(&mut engine, id, 2), None);
    assert_eq!(engine.host_state.logs.len(), 1);

    // Recovery
    let safe_dt = DateTime::parse_from_rfc3339("2026-03-15T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(safe_dt)))
        .unwrap();
    assert!(pull(&mut engine, id, 2).is_some());
}

#[test]
fn shift_time_time_of_day_wraps_around_midnight() {
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "time/shift",
        &[("value_type".into(), "time".into())],
    );

    let t = NaiveTime::from_hms_opt(23, 30, 0).unwrap();
    let dur = TimeDelta::hours(2);

    engine
        .set_input_value(id, PinId(0), Some(DataValue::Time(t)))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Duration(dur)))
        .unwrap();

    // 23:30 + 2h = 01:30 (midnight wrap forward)
    let expected_wrap_add = NaiveTime::from_hms_opt(1, 30, 0).unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Time(expected_wrap_add))
    );

    // 23:30 - 2h = 21:30
    engine
        .set_node_property(id, "operation", "subtract".into())
        .unwrap();
    let expected_sub = NaiveTime::from_hms_opt(21, 30, 0).unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Time(expected_sub))
    );

    // 00:30 - 2h = 22:30 (midnight wrap backward)
    let early = NaiveTime::from_hms_opt(0, 30, 0).unwrap();
    engine
        .set_input_value(id, PinId(0), Some(DataValue::Time(early)))
        .unwrap();
    let expected_wrap_sub = NaiveTime::from_hms_opt(22, 30, 0).unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Time(expected_wrap_sub))
    );

    // Negative duration: 00:30 + (-2h) = 22:30
    engine
        .set_node_property(id, "operation", "add".into())
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Duration(-dur)))
        .unwrap();
    assert_eq!(
        pull(&mut engine, id, 2),
        Some(DataValue::Time(expected_wrap_sub))
    );
}

#[test]
fn break_datetime_known_instant() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "time/break", &[]);

    let naive_date = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
    let naive_time = NaiveTime::from_hms_opt(14, 30, 45).unwrap();
    let naive_dt = naive_date.and_time(naive_time);
    let local_dt = chrono::Local
        .from_local_datetime(&naive_dt)
        .earliest()
        .unwrap();
    let utc_dt = local_dt.with_timezone(&Utc);

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(utc_dt)))
        .unwrap();

    assert_eq!(pull(&mut engine, id, 1), Some(DataValue::from(2026i64)));
    assert_eq!(pull(&mut engine, id, 2), Some(DataValue::from(7i64)));
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::from(15i64)));
    assert_eq!(
        pull(&mut engine, id, 4),
        Some(DataValue::from(
            local_dt.weekday().number_from_monday() as i64
        ))
    );
    assert_eq!(pull(&mut engine, id, 5), Some(DataValue::from(14i64)));
    assert_eq!(pull(&mut engine, id, 6), Some(DataValue::from(30i64)));
    assert_eq!(pull(&mut engine, id, 7), Some(DataValue::from(45i64)));
    assert_eq!(pull(&mut engine, id, 8), Some(DataValue::Time(naive_time)));
    assert_eq!(
        pull(&mut engine, id, 9),
        Some(DataValue::from(local_dt.ordinal() as i64))
    );
}

#[test]
fn start_of_each_unit() {
    let units = [
        ("minute", 2026, 7, 15, 14, 35, 0),
        ("hour", 2026, 7, 15, 14, 0, 0),
        ("day", 2026, 7, 15, 0, 0, 0),
        ("week", 2026, 7, 13, 0, 0, 0), // 2026-07-15 is Wednesday; Monday is 2026-07-13
        ("month", 2026, 7, 1, 0, 0, 0),
        ("year", 2026, 1, 1, 0, 0, 0),
    ];

    let naive_dt = NaiveDate::from_ymd_opt(2026, 7, 15)
        .unwrap()
        .and_hms_opt(14, 35, 45)
        .unwrap();
    let local_dt = chrono::Local
        .from_local_datetime(&naive_dt)
        .earliest()
        .unwrap();
    let utc_dt = local_dt.with_timezone(&Utc);

    for (unit, y, m, d, h, min, s) in units {
        let mut engine = TickEngine::new();
        let id = add(
            &mut engine,
            "time/start_of",
            &[("unit".into(), unit.into())],
        );
        engine
            .set_input_value(id, PinId(0), Some(DataValue::DateTime(utc_dt)))
            .unwrap();

        let expected_naive = NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, s)
            .unwrap();
        let expected_local = chrono::Local
            .from_local_datetime(&expected_naive)
            .earliest()
            .unwrap();
        let expected_utc = expected_local.with_timezone(&Utc);

        assert_eq!(
            pull(&mut engine, id, 1),
            Some(DataValue::DateTime(expected_utc)),
            "start_of unit {unit}"
        );
    }
}

#[test]
fn in_range_normal_crossing_midnight_equal_bounds_and_using_now() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "time/in_range", &[]);

    let from = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let to = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(from)))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(DataValue::Time(to)))
        .unwrap();

    let make_dt = |h, m, s| {
        let naive = NaiveDate::from_ymd_opt(2026, 5, 20)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap();
        chrono::Local
            .from_local_datetime(&naive)
            .earliest()
            .unwrap()
            .with_timezone(&Utc)
    };

    // Normal range [09:00, 17:00)
    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(12, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(true)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(9, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(true)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(17, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(8, 59, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));

    // Crossing midnight [22:00, 06:00)
    let night_from = NaiveTime::from_hms_opt(22, 0, 0).unwrap();
    let night_to = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(night_from)))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(DataValue::Time(night_to)))
        .unwrap();

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(23, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(true)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(2, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(true)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(12, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));

    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(6, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));

    // Equal bounds (empty range)
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(night_from)))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(DataValue::Time(night_from)))
        .unwrap();
    engine
        .set_input_value(id, PinId(0), Some(DataValue::DateTime(make_dt(22, 0, 0))))
        .unwrap();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));

    // Fall back to host_state.now() when Value is unconnected
    engine.set_input_value(id, PinId(0), None).unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::Time(from)))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(DataValue::Time(to)))
        .unwrap();

    engine.host_state.clock_override = Some(make_dt(14, 0, 0));
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(true)));

    engine.host_state.clock_override = Some(make_dt(20, 0, 0));
    engine.tick();
    assert_eq!(pull(&mut engine, id, 3), Some(DataValue::Bool(false)));
}
