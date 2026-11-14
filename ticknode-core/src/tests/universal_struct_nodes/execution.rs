use super::*;
use chrono::Utc;
use std::collections::BTreeMap;

fn get_data(e: &mut TickEngine, n: NodeId, p: PinId) -> Option<DataValue> {
    e.pull_data(n, p, &mut Vec::new())
}

#[test]
fn roundtrip_with_and_without_optional_fields_and_identifiers() {
    let mut engine = setup_engine();

    // 1. telegram.user WITH optional fields
    let make_user = NodeId::generate();
    let break_user = NodeId::generate();
    engine
        .create_node(
            make_user,
            "object/make",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .create_node(
            break_user,
            "object/break",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .connect_data(make_user, PinId(0), break_user, PinId(0))
        .unwrap();

    // Provide values for all fields:
    // 1: id (telegram.user_id)
    // 2: first_name (String)
    // 3: last_name (String, optional)
    // 4: username (String, optional)
    // 5: language (String, optional)
    // 6: is_bot (Bool)
    // 1: id (telegram.user_id) - identifier inline value
    engine
        .set_input_value(make_user, PinId(1), Some(12345i64.into()))
        .unwrap();

    let c2 = NodeId::generate();
    let c3 = NodeId::generate();
    let c4 = NodeId::generate();
    let c5 = NodeId::generate();
    let c6 = NodeId::generate();
    engine
        .create_node(c2, "const", &[("value".into(), "Pavlo".into())])
        .unwrap();
    engine
        .create_node(c3, "const", &[("value".into(), "Ver".into())])
        .unwrap();
    engine
        .create_node(c4, "const", &[("value".into(), "pavver".into())])
        .unwrap();
    engine
        .create_node(c5, "const", &[("value".into(), "uk".into())])
        .unwrap();
    engine
        .create_node(c6, "const", &[("value".into(), false.into())])
        .unwrap();

    engine
        .connect_data(c2, PinId(0), make_user, PinId(2))
        .unwrap();
    engine
        .connect_data(c3, PinId(0), make_user, PinId(3))
        .unwrap();
    engine
        .connect_data(c4, PinId(0), make_user, PinId(4))
        .unwrap();
    engine
        .connect_data(c5, PinId(0), make_user, PinId(5))
        .unwrap();
    engine
        .connect_data(c6, PinId(0), make_user, PinId(6))
        .unwrap();

    // Sinks for break_user outputs
    let mut sinks = Vec::new();
    for pin in 1..=6 {
        let sink = NodeId::generate();
        engine.create_node(sink, "test/debug", &[]).unwrap();
        engine
            .connect_data(break_user, PinId(pin), sink, PinId(1))
            .unwrap();
        sinks.push(sink);
    }

    let clock = NodeId::generate();
    engine.create_node(clock, "clock", &[]).unwrap();
    engine
        .connect_exec(clock, PinId(0), sinks[0], PinId(0))
        .unwrap();
    engine.tick();

    assert_eq!(
        get_data(&mut engine, break_user, PinId(1)),
        Some(DataValue::Number(NumberValue::Int(12345)))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(2)),
        Some(DataValue::String("Pavlo".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(3)),
        Some(DataValue::String("Ver".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(4)),
        Some(DataValue::String("pavver".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(5)),
        Some(DataValue::String("uk".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(6)),
        Some(DataValue::Bool(false))
    );

    // 2. Disconnect optional fields 3, 4, 5
    engine.disconnect_data(make_user, PinId(3));
    engine.disconnect_data(make_user, PinId(4));
    engine.disconnect_data(make_user, PinId(5));

    engine.tick();

    // Required fields still present:
    assert_eq!(
        get_data(&mut engine, break_user, PinId(1)),
        Some(DataValue::Number(NumberValue::Int(12345)))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(2)),
        Some(DataValue::String("Pavlo".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_user, PinId(6)),
        Some(DataValue::Bool(false))
    );
    // Optional omitted fields give no output (cleared):
    assert_eq!(get_data(&mut engine, break_user, PinId(3)), None);
    assert_eq!(get_data(&mut engine, break_user, PinId(4)), None);
    assert_eq!(get_data(&mut engine, break_user, PinId(5)), None);
}

#[test]
fn nested_struct_roundtrip() {
    let mut engine = setup_engine();

    // Build telegram.user
    let make_user = NodeId::generate();
    engine
        .create_node(
            make_user,
            "object/make",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .set_input_value(make_user, PinId(1), Some(99i64.into()))
        .unwrap();
    let u_name = NodeId::generate();
    let u_bot = NodeId::generate();
    engine
        .create_node(u_name, "const", &[("value".into(), "BotFather".into())])
        .unwrap();
    engine
        .create_node(u_bot, "const", &[("value".into(), true.into())])
        .unwrap();
    engine
        .connect_data(u_name, PinId(0), make_user, PinId(2))
        .unwrap();
    engine
        .connect_data(u_bot, PinId(0), make_user, PinId(6))
        .unwrap();

    // Build telegram.message with nested from = telegram.user
    let make_msg = NodeId::generate();
    engine
        .create_node(
            make_msg,
            "object/make",
            &[("type".into(), "telegram.message".into())],
        )
        .unwrap();

    // Connect make_user to make_msg pin 4 (from)
    engine
        .connect_data(make_user, PinId(0), make_msg, PinId(4))
        .unwrap();

    // Required fields of telegram.message:
    // 1: chat (telegram.chat_id)
    // 3: id (telegram.message_id)
    // 6: date (DateTime)
    // 9: chat_type (String)
    engine
        .set_input_value(make_msg, PinId(1), Some(1001i64.into()))
        .unwrap();
    engine
        .set_input_value(make_msg, PinId(3), Some(500i64.into()))
        .unwrap();

    let now = Utc::now();
    let m_date = NodeId::generate();
    let m_type = NodeId::generate();
    engine
        .create_node(
            m_date,
            "const",
            &[("value".into(), DataValue::DateTime(now))],
        )
        .unwrap();
    engine
        .create_node(m_type, "const", &[("value".into(), "supergroup".into())])
        .unwrap();

    engine
        .connect_data(m_date, PinId(0), make_msg, PinId(6))
        .unwrap();
    engine
        .connect_data(m_type, PinId(0), make_msg, PinId(9))
        .unwrap();

    // Break telegram.message
    let break_msg = NodeId::generate();
    engine
        .create_node(
            break_msg,
            "object/break",
            &[("type".into(), "telegram.message".into())],
        )
        .unwrap();
    engine
        .connect_data(make_msg, PinId(0), break_msg, PinId(0))
        .unwrap();

    // Break the nested from user
    let break_nested_user = NodeId::generate();
    engine
        .create_node(
            break_nested_user,
            "object/break",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .connect_data(break_msg, PinId(4), break_nested_user, PinId(0))
        .unwrap();

    let sink = NodeId::generate();
    engine.create_node(sink, "test/debug", &[]).unwrap();
    engine
        .connect_data(break_nested_user, PinId(2), sink, PinId(1))
        .unwrap();

    let clock = NodeId::generate();
    engine.create_node(clock, "clock", &[]).unwrap();
    engine
        .connect_exec(clock, PinId(0), sink, PinId(0))
        .unwrap();
    engine.tick();

    assert_eq!(
        get_data(&mut engine, break_msg, PinId(1)),
        Some(DataValue::Number(NumberValue::Int(1001)))
    );
    assert_eq!(
        get_data(&mut engine, break_msg, PinId(9)),
        Some(DataValue::String("supergroup".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_nested_user, PinId(1)),
        Some(DataValue::Number(NumberValue::Int(99)))
    );
    assert_eq!(
        get_data(&mut engine, break_nested_user, PinId(2)),
        Some(DataValue::String("BotFather".into()))
    );
    assert_eq!(
        get_data(&mut engine, break_nested_user, PinId(6)),
        Some(DataValue::Bool(true))
    );
}

#[test]
fn ha_state_roundtrip_with_attributes_and_timestamps() {
    let mut engine = setup_engine();

    let make = NodeId::generate();
    let brk = NodeId::generate();
    engine
        .create_node(make, "object/make", &[("type".into(), "ha.state".into())])
        .unwrap();
    engine
        .create_node(brk, "object/break", &[("type".into(), "ha.state".into())])
        .unwrap();
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();

    // ha.state fields:
    // 1: entity_id (String)
    // 2: state (String)
    // 3: attributes (Object)
    // 4: last_changed (DateTime)
    // 5: last_updated (DateTime)
    let c_entity = NodeId::generate();
    let c_state = NodeId::generate();
    let c_attrs = NodeId::generate();
    let c_changed = NodeId::generate();
    let c_updated = NodeId::generate();

    let now = Utc::now();
    let mut attr_fields = BTreeMap::new();
    attr_fields.insert(
        "friendly_name".to_string(),
        ObjectField {
            name: "friendly_name".into(),
            description: String::new(),
            value: DataValue::String("Living Room Light".into()),
        },
    );
    attr_fields.insert(
        "brightness".to_string(),
        ObjectField {
            name: "brightness".into(),
            description: String::new(),
            value: DataValue::Number(NumberValue::Int(255)),
        },
    );

    engine
        .create_node(
            c_entity,
            "const",
            &[("value".into(), "light.living_room".into())],
        )
        .unwrap();
    engine
        .create_node(c_state, "const", &[("value".into(), "on".into())])
        .unwrap();
    engine
        .create_node(
            c_attrs,
            "const",
            &[(
                "value".into(),
                DataValue::Object {
                    fields: attr_fields,
                    type_name: None,
                },
            )],
        )
        .unwrap();
    engine
        .create_node(
            c_changed,
            "const",
            &[("value".into(), DataValue::DateTime(now))],
        )
        .unwrap();
    engine
        .create_node(
            c_updated,
            "const",
            &[("value".into(), DataValue::DateTime(now))],
        )
        .unwrap();

    engine
        .connect_data(c_entity, PinId(0), make, PinId(1))
        .unwrap();
    engine
        .connect_data(c_state, PinId(0), make, PinId(2))
        .unwrap();
    engine
        .connect_data(c_attrs, PinId(0), make, PinId(3))
        .unwrap();
    engine
        .connect_data(c_changed, PinId(0), make, PinId(4))
        .unwrap();
    engine
        .connect_data(c_updated, PinId(0), make, PinId(5))
        .unwrap();

    assert_eq!(
        get_data(&mut engine, brk, PinId(1)),
        Some(DataValue::String("light.living_room".into()))
    );
    assert_eq!(
        get_data(&mut engine, brk, PinId(2)),
        Some(DataValue::String("on".into()))
    );
    assert_eq!(
        get_data(&mut engine, brk, PinId(4)),
        Some(DataValue::DateTime(now))
    );
    assert_eq!(
        get_data(&mut engine, brk, PinId(5)),
        Some(DataValue::DateTime(now))
    );
    let attrs = get_data(&mut engine, brk, PinId(3));
    assert!(matches!(attrs, Some(DataValue::Object { .. })));
}

#[test]
fn adaptation_on_connect() {
    let mut engine = setup_engine();

    let make = NodeId::generate();
    engine
        .create_node(make, "object/make", &[("type".into(), "ha.state".into())])
        .unwrap();

    let brk = NodeId::generate();
    engine.create_node(brk, "object/break", &[]).unwrap();

    // Before connecting: brk has pin 0 Any, no field pins
    let def_before = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(def_before.pins.len(), 1);
    assert_eq!(def_before.pins[0].pin_type, PinType::Data(DataType::Any));

    // Connect make output 0 to brk input 0
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();

    // After connecting: brk adapted to ha.state!
    let def_after = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(def_after.pins.len(), 6);
    assert_eq!(
        def_after.pins[0].pin_type,
        PinType::Data(DataType::Named("ha.state".into()))
    );
    assert!(def_after.pins[0].required);
    assert_eq!(def_after.pins[1].name, "Entity ID");
    assert_eq!(def_after.pins[2].name, "State");
    assert_eq!(def_after.pins[3].name, "Attributes");
    assert_eq!(def_after.pins[4].name, "Last changed");
    assert_eq!(def_after.pins[5].name, "Last updated");

    // Connecting non-struct type to another unconfigured break node declines adaptation
    let brk2 = NodeId::generate();
    engine.create_node(brk2, "object/break", &[]).unwrap();
    let num_const = NodeId::generate();
    engine
        .create_node(num_const, "const", &[("value".into(), 42i64.into())])
        .unwrap();
    engine
        .connect_data(num_const, PinId(0), brk2, PinId(0))
        .unwrap();
    let def_num = engine.fresh_node_definition(brk2).unwrap();
    // Did not adapt: still 1 pin Any
    assert_eq!(def_num.pins.len(), 1);
    assert_eq!(def_num.pins[0].pin_type, PinType::Data(DataType::Any));
}
