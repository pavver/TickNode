use std::time::Duration;
use ticknode_shared::condition::{CompareOp, Condition, Join, MatchCase, Operand};
use ticknode_shared::{
    DataType, DataValue, NodeId, NodeTimer, ServerMessage, TypeRegistry, input_value_fits, text,
};

#[test]
fn ticks_parse_format_coerce_and_round_trip() {
    let types = TypeRegistry::new();
    for suffix in [
        "t",
        "tick",
        "ticks",
        "такт",
        "такти",
        "тактів",
        "TICKS",
        "ТАКТІВ",
    ] {
        for gap in ["", " "] {
            let input = format!(" 10{gap}{suffix} ");
            let value = text::parse_value(&input, &DataType::Duration).unwrap();
            assert_eq!(value, DataValue::Ticks(10));
            assert_eq!(value.get_type(), DataType::Duration);
            assert!(input_value_fits(&types, &DataType::Duration, &value));
            assert_eq!(text::format_value(&value), "10t");
            assert_eq!(
                text::coerce_to(DataValue::String(input), &DataType::Duration),
                value
            );
            let encoded = postcard::to_allocvec(&value).unwrap();
            assert_eq!(postcard::from_bytes::<DataValue>(&encoded).unwrap(), value);
            let operand = Operand::Value(value);
            let case = MatchCase {
                pin: 1,
                conditions: vec![Condition {
                    left: Operand::Input("delay".into()),
                    op: CompareOp::Eq,
                    right: Some(operand),
                    join: Join::And,
                }],
            };
            assert!(
                MatchCase::parse(&case.to_text())
                    .unwrap()
                    .matches(|_| Some(DataValue::Ticks(10)))
            );
        }
    }
    for text in [
        "-1t",
        "+1t",
        "1.5t",
        "t",
        "1t 2s",
        "1s 2t",
        "18446744073709551616t",
    ] {
        assert!(
            text::parse_value(text, &DataType::Duration).is_err(),
            "{text}"
        );
    }
    for n in [0, u64::MAX] {
        assert_eq!(
            text::parse_value(
                &text::format_value(&DataValue::Ticks(n)),
                &DataType::Duration
            )
            .unwrap(),
            DataValue::Ticks(n)
        );
    }
    assert_eq!(
        text::default_of(&DataType::Duration),
        DataValue::Duration(Duration::from_secs(1))
    );
}

#[test]
fn duration_connections_convert_ticks_and_lists() {
    let types = TypeRegistry::new();
    let parse = types
        .convertible(&DataType::String, &DataType::Duration)
        .unwrap();
    let format = types
        .convertible(&DataType::Duration, &DataType::String)
        .unwrap();
    assert_eq!(parse.apply("10 тактів".into()), Some(DataValue::Ticks(10)));
    assert_eq!(format.apply(DataValue::Ticks(10)), Some("10t".into()));
    assert_eq!(
        parse.apply("5s".into()),
        Some(DataValue::Duration(Duration::from_secs(5)))
    );
    let conversion = types
        .convertible(
            &DataType::List(Box::new(DataType::String)),
            &DataType::List(Box::new(DataType::Duration)),
        )
        .unwrap();
    let list = |elements| DataValue::List {
        element_type: DataType::String,
        elements,
    };
    assert_eq!(
        conversion.apply(list(vec!["2t".into(), "1s".into()])),
        Some(DataValue::List {
            element_type: DataType::Duration,
            elements: vec![
                DataValue::Ticks(2),
                DataValue::Duration(Duration::from_secs(1))
            ],
        })
    );
    assert_eq!(
        conversion.apply(list(vec!["2t".into(), "invalid".into()])),
        None
    );
}

#[test]
fn match_compares_only_durations_with_the_same_unit_basis() {
    use CompareOp::*;
    let time = DataValue::Duration(Duration::from_secs(10));
    for op in [Eq, Ne, Lt, Le, Gt, Ge] {
        let holds = |a: DataValue, b: DataValue| {
            Condition {
                left: Operand::Value(a),
                op,
                right: Some(Operand::Value(b)),
                join: Join::And,
            }
            .holds(&|_| None)
        };
        assert!(!holds(DataValue::Ticks(10), time.clone()), "{op:?}");
        assert!(!holds(time.clone(), DataValue::Ticks(10)), "{op:?}");
        // Coercion must not turn a mixed duration comparison into inequality.
        assert!(!holds(DataValue::Ticks(10), "10s".into()), "{op:?}");
        assert!(!holds(time.clone(), "10t".into()), "{op:?}");
        assert_eq!(
            holds(DataValue::Ticks(10), DataValue::Ticks(10)),
            matches!(op, Eq | Le | Ge)
        );
        assert_eq!(
            holds(DataValue::Ticks(10), DataValue::Ticks(20)),
            matches!(op, Ne | Lt | Le)
        );
        assert_eq!(
            holds(time.clone(), time.clone()),
            matches!(op, Eq | Le | Ge)
        );
    }
}

#[test]
fn timer_message_round_trips() {
    let message = ServerMessage::Timers(vec![NodeTimer {
        node: NodeId::generate(),
        total_ms: 1000,
        remaining_ms: 700,
        count: 2,
    }]);
    assert_eq!(
        postcard::from_bytes::<ServerMessage>(&postcard::to_allocvec(&message).unwrap()).unwrap(),
        message
    );
}
