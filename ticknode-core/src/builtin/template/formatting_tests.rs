use super::{
    tests::{connect_value, inline, pins, render, setup},
    *,
};
use crate::TickEngine;
use ticknode_shared::quantity::{QuantityDisplay, Rounding, UnitStep};

fn connect_power(engine: &mut TickEngine, id: NodeId, name: &str) {
    let source = NodeId::generate();
    engine
        .create_node(
            source,
            "const",
            &[
                ("value_type".into(), "quantity.power".into()),
                ("value".into(), 1200.into()),
            ],
        )
        .unwrap();
    let pin = pins(engine, id)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1;
    engine.connect_data(source, PinId(0), id, pin).unwrap();
}

#[test]
fn text_map_keeps_source_type_while_filter_uses_only_runtime_kind() {
    let (mut engine, id) =
        setup("{{ text.power }}|{{ text.number }}|{{ power / 1000 }}|{{ power | text }}");
    connect_power(&mut engine, id, "power");
    inline(&mut engine, id, "number", 1200.into());
    assert_eq!(render(&mut engine, id), Some("1.2 kW|1200|1.2|1200".into()));
    engine.set_locale("uk");
    assert_eq!(
        render(&mut engine, id),
        Some("1,2 кВт|1200|1.2|1200".into())
    );
    let mut display = DisplayRules::default();
    display.quantities.insert(
        "quantity.power".into(),
        QuantityDisplay {
            steps: vec![UnitStep {
                from: 0.0,
                unit: "watt".into(),
            }],
            digits: 2,
            rounding: Rounding::Decimals,
        },
    );
    engine.set_display_rules(display);
    assert_eq!(
        render(&mut engine, id),
        Some("1200,00 Вт|1200|1.2|1200".into())
    );
}

#[test]
fn text_map_matches_to_text_for_every_value_kind_in_both_locales() {
    let cases = vec![
        true.into(),
        42.into(),
        DataValue::Number(NumberValue::Float(12.3456)),
        "hello".into(),
        text::parse_value("13:14:15", &DataType::Time).unwrap(),
        text::parse_value("2025-01-02T03:04:05Z", &DataType::DateTime).unwrap(),
        text::parse_value("1h 30m", &DataType::Duration).unwrap(),
        DataValue::Ticks(3),
        text::parse_value("#ff0000", &DataType::Color).unwrap(),
        DataValue::List {
            elements: vec![true.into(), false.into()],
            element_type: DataType::Bool,
        },
        DataValue::Object {
            type_name: None,
            fields: [(
                "n".into(),
                ObjectField {
                    name: "n".into(),
                    description: String::new(),
                    value: 123.into(),
                },
            )]
            .into(),
        },
    ];
    for locale in ["en", "uk"] {
        for value in &cases {
            let (mut engine, id) = setup("{{ text.x }}");
            engine.set_locale(locale);
            connect_value(&mut engine, id, "x", value.clone());
            let expected = ticknode_shared::to_text::to_text_in(
                value,
                &value.get_type(),
                &Default::default(),
                &engine.host_state.display,
                crate::quantity::labels(locale),
                &engine.types,
            );
            assert_eq!(
                render(&mut engine, id),
                Some(expected.into()),
                "{locale}: {value:?}"
            );
        }
    }
}

#[test]
fn list_and_struct_field_formatting_keeps_declared_quantity_types() {
    let (mut engine, id) = setup(
        "{{ text.powers }}|{{ text.sensor }}|\
        {% for item in powers %}{{ item | text }};{% endfor %}{{ sensor.power | text }}",
    );
    let quantity = DataType::Named("quantity.power".into());
    connect_value(
        &mut engine,
        id,
        "powers",
        DataValue::List {
            element_type: quantity.clone(),
            elements: vec![1200.into(), 50.into()],
        },
    );
    engine
        .register_types(
            &[TypeDefinition::new_struct(
                "example.sensor",
                "Sensor",
                vec![FieldDefinition::new("power", "Power", quantity)],
            )],
            &[],
        )
        .unwrap();
    connect_value(
        &mut engine,
        id,
        "sensor",
        DataValue::Object {
            type_name: Some("example.sensor".into()),
            fields: [(
                "power".into(),
                ObjectField {
                    name: "power".into(),
                    description: String::new(),
                    value: 1200.into(),
                },
            )]
            .into(),
        },
    );
    assert_eq!(
        render(&mut engine, id),
        Some("1.2 kW, 50 W|{power: 1.2 kW}|1200;50;1200".into())
    );
    engine.set_locale("uk");
    assert_eq!(
        render(&mut engine, id),
        Some("1,2 кВт, 50 Вт|{power: 1,2 кВт}|1200;50;1200".into())
    );
}

#[test]
fn runtime_text_filter_formats_computed_values_recursively_without_source_units() {
    let (mut engine, id) = setup(
        "{{ (power / 1000) | text }}|{{ on | text }}|{{ state | text }}|\
        {{ [on, false, power / 1000, none] | text }}|\
        {{ {'enabled': on, 'name': state, 'values': [power / 1000, false]} | text }}",
    );
    connect_power(&mut engine, id, "power");
    inline(&mut engine, id, "on", true.into());
    inline(&mut engine, id, "state", "on".into());
    for (locale, expected) in [
        (
            "en",
            "1.2|true|on|true, false, 1.2, |{enabled: true, name: \"on\", values: 1.2, false}",
        ),
        (
            "uk",
            "1,2|так|on|так, ні, 1,2, |{enabled: так, name: \"on\", values: 1,2, ні}",
        ),
    ] {
        engine.set_locale(locale);
        assert_eq!(render(&mut engine, id), Some(expected.into()), "{locale}");
    }
    let mut display = DisplayRules::default();
    display.number = NumberDisplay {
        rounding: Rounding::Decimals,
        digits: 2,
    };
    engine.set_display_rules(display);
    assert_eq!(
        render(&mut engine, id),
        Some("1,20|так|on|так, ні, 1,20, |{enabled: так, name: \"on\", values: 1,20, ні}".into())
    );
}

#[test]
fn runtime_error_recovers_from_upstream_changes_without_editing_template() {
    let (mut engine, id) = setup("{% if fail %}{{ 1 // 0 }}{% else %}ok{% endif %}");
    let source = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), true.into())])
        .unwrap();
    engine.connect_data(source, PinId(0), id, PinId(1)).unwrap();
    assert_eq!(render(&mut engine, id), None);
    assert_eq!(engine.host_state.logs.len(), 1);
    engine.tick();
    engine
        .set_node_property(source, "value", false.into())
        .unwrap();
    assert_eq!(render(&mut engine, id), Some("ok".into()));
    assert!(engine.node_errors().is_empty());
    engine.tick();
    engine
        .set_node_property(source, "value", true.into())
        .unwrap();
    assert_eq!(render(&mut engine, id), None);
    assert_eq!(engine.host_state.logs.len(), 2);
}

#[test]
fn runtime_text_filter_preserves_large_computed_integers() {
    let (mut engine, id) = setup("{{ (n + 1) | text }}");
    inline(
        &mut engine,
        id,
        "n",
        DataValue::Number(NumberValue::Int(i64::MAX)),
    );
    assert_eq!(render(&mut engine, id), Some("9223372036854775808".into()));
    engine.set_locale("uk");
    let mut display = DisplayRules::default();
    display.number = NumberDisplay {
        rounding: Rounding::Decimals,
        digits: 2,
    };
    engine.set_display_rules(display);
    assert_eq!(
        render(&mut engine, id),
        Some("9223372036854775808,00".into())
    );
}
