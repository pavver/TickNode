use super::*;

fn ends(exec: bool) -> (WireEnd, WireEnd) {
    (WireEnd { node: NodeId::generate(), pin: PinId(1), dir: PinDirection::Output, exec },
     WireEnd { node: NodeId::generate(), pin: PinId(2), dir: PinDirection::Input, exec })
}

#[test]
fn paste_and_mouse_buttons_keep_their_precedence() {
    assert_eq!(press(true, "touch", 0), Press::Place);
    assert_eq!(press(true, "mouse", 2), Press::Cancel);
    assert_eq!(press(true, "mouse", 3), Press::Ignore);
    assert_eq!(press(true, "mouse", 1), Press::Continue);
    assert_eq!(press(false, "mouse", 2), Press::Ignore);
    assert_eq!(press(false, "touch", 2), Press::Continue);
    assert_eq!(press(false, "mouse", 0), Press::Continue);
}

#[test]
fn gestures_prioritize_pan_then_pin_then_wire_then_node_then_background() {
    let (pin, _) = ends(false);
    let connection = ConnectionModel { src_node: pin.node, src_pin: pin.pin, dst_node: NodeId::generate(), dst_pin: PinId(3) };
    let wire = Selection::Data(connection);
    assert_eq!(gesture(1, "mouse", true, Some(pin), Some(wire.clone()), Some(pin.node)), Gesture::Pan { moved: true });
    assert_eq!(gesture(0, "touch", true, Some(pin), Some(wire.clone()), Some(pin.node)), Gesture::Wire(pin));
    assert_eq!(gesture(0, "touch", true, None, Some(wire.clone()), Some(pin.node)), Gesture::Select(wire));
    assert_eq!(gesture(0, "touch", true, None, None, Some(pin.node)), Gesture::Toggle(pin.node));
    assert_eq!(gesture(0, "touch", false, None, None, Some(pin.node)), Gesture::Nodes(pin.node));
    assert_eq!(gesture(0, "touch", false, None, None, None), Gesture::Pan { moved: false });
    assert_eq!(gesture(0, "mouse", false, None, None, None), Gesture::Box);
}

#[test]
fn wire_attributes_validate_ids_and_pins_and_keep_unknown_kind_as_data() {
    let (from, to) = ends(false);
    let a = from.node.0.to_string();
    let b = to.node.0.to_string();
    let c = ConnectionModel { src_node: from.node, src_pin: PinId(1), dst_node: to.node, dst_pin: PinId(2) };
    assert_eq!(wire_selection(&a, "1", &b, "2", "exec"), Some(Selection::Exec(c)));
    assert_eq!(wire_selection(&a, "1", &b, "2", "unknown"), Some(Selection::Data(c)));
    assert_eq!(wire_selection("bad", "1", &b, "2", "exec"), None);
    assert_eq!(wire_selection(&a, "bad", &b, "2", "exec"), None);
    assert_eq!(wire_selection(&a, "1", "bad", "2", "exec"), None);
    assert_eq!(wire_selection(&a, "1", &b, "-1", "exec"), None);
}

#[test]
fn wire_drags_send_identical_commands_from_either_end() {
    for exec in [false, true] {
        let (from, to) = ends(exec);
        let c = ConnectionModel { src_node: from.node, src_pin: from.pin, dst_node: to.node, dst_pin: to.pin };
        let expected = if exec { ClientMessage::ConnectExec(c) } else { ClientMessage::ConnectData(c) };
        for (a, b) in [(from, to), (to, from)] {
            assert_eq!(wire_command(a, b).unwrap(), Some(expected.clone()));
        }
        assert_eq!(wire_command(from, from), Ok(None));
        assert_eq!(wire_command(from, WireEnd { dir: from.dir, ..to }), Err("[ERROR] Only an output can be connected to an input"));
        assert_eq!(wire_command(from, WireEnd { exec: !exec, ..to }), Err("[ERROR] An exec pin can only be connected to an exec pin"));
    }
}
