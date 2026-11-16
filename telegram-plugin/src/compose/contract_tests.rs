use super::*;
use crate::{outgoing, schema};
use ticknode_plugin_sdk::*;

fn keyboard(node: &mut KeyboardNode, inputs: &Inputs) -> outgoing::Keyboard {
    let outputs = node.execute(Trigger::Pin(PinId(0)), inputs);
    let [Output::Data(pin, value)] = outputs.as_slice() else {
        panic!("one keyboard output")
    };
    assert_eq!(*pin, PinId(0));
    outgoing::parse_keyboard(value).unwrap()
}

#[test]
fn restored_keyboard_geometry_stays_bounded_and_ignores_output_pin_inputs() {
    let inputs = [
        (PinId(0), "ignored".into()),
        (PinId(1), "First".into()),
        (PinId(2), "Second".into()),
    ]
    .into_iter()
    .collect();
    let mut node = KeyboardNode::new();
    for count in ["0", "1"] {
        node.set_property("count", count.into());
        for columns in ["0", "invalid", "1"] {
            node.set_property("columns", columns.into());
            let value = keyboard(&mut node, &inputs);
            assert_eq!(value.rows.len(), 1);
            assert_eq!(value.rows[0].len(), 1);
            assert_eq!(value.rows[0][0].text, "First");
        }
    }
    node.set_property("kind", "remove".into());
    let value = keyboard(&mut node, &inputs);
    assert_eq!(value.kind, outgoing::KeyboardKind::Remove);
    assert_eq!(
        outgoing::keyboard_markup(&value),
        serde_json::json!({"remove_keyboard":true})
    );

    let props = std::collections::BTreeMap::from([("count".into(), "0".into())]);
    let def = schema::action_definition("keyboard", &props);
    assert!(
        def.pins
            .iter()
            .any(|p| p.id == PinId(1) && p.direction == PinDirection::Input)
    );
    for kind in ["inline", "reply", "remove"] {
        let props = std::collections::BTreeMap::from([("kind".into(), kind.into())]);
        let def = schema::action_definition("keyboard", &props);
        let resize = def.properties.iter().find(|p| p.name == "resize").unwrap();
        assert_eq!(resize.kind != PropertyKind::Hidden, kind == "reply");
    }
}

#[test]
fn composed_messages_leave_the_builder_on_the_documented_output_pin() {
    let mut node = ComposeNode::new();
    let inputs = [(PinId(1), "hello".into())].into_iter().collect();
    let outputs = node.execute(Trigger::Pin(PinId(0)), &inputs);
    let [Output::Data(pin, value)] = outputs.as_slice() else {
        panic!("one message output")
    };
    assert_eq!(*pin, PinId(0));
    assert_eq!(
        outgoing::parse_outgoing(value).unwrap().text.as_deref(),
        Some("hello")
    );
}
