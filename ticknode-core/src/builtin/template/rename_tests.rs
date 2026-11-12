use super::tests::{connect_value, inline, pins, render, setup};
use crate::TickEngine;
use ticknode_shared::*;

#[test]
fn single_variable_rename_keeps_pin_and_connected_wire_and_surrounding_inline_values() {
    let (mut engine, id) = setup("Hello {{ value }} {{ suffix }}");
    connect_value(&mut engine, id, "value", "World".into());
    inline(&mut engine, id, "suffix", "!".into());
    assert_eq!(render(&mut engine, id), Some("Hello World !".into()));

    let removed = engine
        .set_node_property(id, "template", "Hello {{ name }} {{ suffix }}".into())
        .unwrap();
    assert!(removed.data.is_empty());
    assert!(removed.exec.is_empty());
    assert_eq!(
        pins(&engine, id),
        vec![("name".into(), PinId(1)), ("suffix".into(), PinId(2))]
    );
    assert_eq!(render(&mut engine, id), Some("Hello World !".into()));
}

#[test]
fn single_input_variable_rename_keeps_pin_and_connected_wire() {
    let (mut engine, id) = setup("Hello {{ value }}");
    connect_value(&mut engine, id, "value", "World".into());
    assert_eq!(render(&mut engine, id), Some("Hello World".into()));

    let removed = engine
        .set_node_property(id, "template", "Hello {{ name }}".into())
        .unwrap();
    assert!(removed.data.is_empty());
    assert!(removed.exec.is_empty());
    assert_eq!(pins(&engine, id), vec![("name".into(), PinId(1))]);
    assert_eq!(render(&mut engine, id), Some("Hello World".into()));
}

#[test]
fn inline_value_survives_a_rename() {
    let (mut engine, id) = setup("Hello {{ value }}");
    inline(&mut engine, id, "value", "Alice".into());
    assert_eq!(render(&mut engine, id), Some("Hello Alice".into()));

    let removed = engine
        .set_node_property(id, "template", "Hello {{ name }}".into())
        .unwrap();
    assert!(removed.data.is_empty());
    assert!(removed.exec.is_empty());
    assert_eq!(pins(&engine, id), vec![("name".into(), PinId(1))]);
    assert!(
        engine
            .node_model(id)
            .unwrap()
            .properties
            .iter()
            .any(|(n, v)| n == "in:1" && *v == DataValue::from("Alice"))
    );
    assert_eq!(render(&mut engine, id), Some("Hello Alice".into()));
}

#[test]
fn rename_rules_for_substitutions_multiple_removals_and_index_mismatch() {
    // {{ a }}{{ b }} -> {{ a }}{{ x }} keeps b's id for x
    let (mut engine, id) = setup("{{ a }}{{ b }}");
    assert_eq!(
        pins(&engine, id),
        vec![("a".into(), PinId(1)), ("b".into(), PinId(2))]
    );
    engine
        .set_node_property(id, "template", "{{ a }}{{ x }}".into())
        .unwrap();
    assert_eq!(
        pins(&engine, id),
        vec![("a".into(), PinId(1)), ("x".into(), PinId(2))]
    );

    // {{ a }}{{ b }} -> {{ x }}{{ y }} (two removed) gets fresh ids
    let (mut engine, id) = setup("{{ a }}{{ b }}");
    assert_eq!(
        pins(&engine, id),
        vec![("a".into(), PinId(1)), ("b".into(), PinId(2))]
    );
    engine
        .set_node_property(id, "template", "{{ x }}{{ y }}".into())
        .unwrap();
    assert_eq!(
        pins(&engine, id),
        vec![("x".into(), PinId(3)), ("y".into(), PinId(4))]
    );

    // index mismatch ({{ b }}{{ a }} -> {{ a }}{{ c }}) gets a fresh id
    let (mut engine, id) = setup("{{ b }}{{ a }}");
    assert_eq!(
        pins(&engine, id),
        vec![("b".into(), PinId(1)), ("a".into(), PinId(2))]
    );
    engine
        .set_node_property(id, "template", "{{ a }}{{ c }}".into())
        .unwrap();
    assert_eq!(
        pins(&engine, id),
        vec![("a".into(), PinId(2)), ("c".into(), PinId(3))]
    );
}

#[test]
fn rename_chain_keeps_one_id() {
    let (mut engine, id) = setup("{{ value }}");
    assert_eq!(pins(&engine, id), vec![("value".into(), PinId(1))]);
    inline(&mut engine, id, "value", "test".into());

    for name in ["v", "va", "val"] {
        engine
            .set_node_property(id, "template", format!("{{{{ {name} }}}}").into())
            .unwrap();
        assert_eq!(pins(&engine, id), vec![(name.into(), PinId(1))]);
        assert_eq!(render(&mut engine, id), Some("test".into()));
    }
}

#[test]
fn restoration_does_not_rename_pins() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node(
            id,
            "text/template",
            &[
                ("template".into(), "{{ new_name }}".into()),
                (
                    "pin_ids".into(),
                    DataValue::List {
                        element_type: DataType::String,
                        elements: vec!["1:old_name".into()],
                    },
                ),
                ("next_pin".into(), 2i64.into()),
            ],
        )
        .unwrap();
    assert_eq!(pins(&engine, id), vec![("new_name".into(), PinId(2))]);
}
