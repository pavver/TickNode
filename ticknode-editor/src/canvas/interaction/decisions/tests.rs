use super::*;

#[test]
fn node_click_action_truth_table() {
    for button in [-1, 0, 1, 2, 3, 4] {
        for shift in [false, true] {
            for ctrl in [false, true] {
                for copied in [None, Some("math/expression"), Some("const")] {
                    let expected = match (button, shift, ctrl, copied) {
                        (2, true, _, _) => NodeClick::CopySettings,
                        (0, _, true, _) => NodeClick::ToggleSelect,
                        (0, true, false, Some("math/expression")) => NodeClick::PasteSettings,
                        (0, true, false, _) => NodeClick::ToggleSelect,
                        (0, false, false, _) => NodeClick::Select,
                        _ => NodeClick::Ignore,
                    };
                    assert_eq!(
                        node_click_action(button, shift, ctrl, copied, "math/expression"),
                        expected,
                        "button={button}, shift={shift}, ctrl/cmd={ctrl}, copied={copied:?}"
                    );
                }
            }
        }
    }
}

fn ends(exec: bool) -> (WireEnd, WireEnd) {
    (
        WireEnd {
            node: NodeId::generate(),
            pin: PinId(1),
            dir: PinDirection::Output,
            exec,
        },
        WireEnd {
            node: NodeId::generate(),
            pin: PinId(2),
            dir: PinDirection::Input,
            exec,
        },
    )
}

#[test]
fn paste_and_mouse_buttons_keep_their_precedence() {
    assert_eq!(press(true, "touch", 0), Press::Place);
    assert_eq!(press(true, "mouse", 2), Press::Cancel);
    assert_eq!(press(true, "mouse", 3), Press::Ignore);
    assert_eq!(press(true, "mouse", 1), Press::Continue);
    assert_eq!(press(false, "mouse", 2), Press::Continue);
    assert_eq!(press(false, "mouse", 3), Press::Ignore);
    assert_eq!(press(false, "touch", 2), Press::Continue);
    assert_eq!(press(false, "mouse", 0), Press::Continue);
}

#[test]
fn gestures_prioritize_pan_then_pin_then_wire_then_node_then_background() {
    let (pin, _) = ends(false);
    let connection = ConnectionModel {
        src_node: pin.node,
        src_pin: pin.pin,
        dst_node: NodeId::generate(),
        dst_pin: PinId(3),
    };
    let wire = Selection::Data(connection);
    let frame_id = FrameId::generate();
    assert_eq!(
        gesture(
            1,
            "mouse",
            Some(pin),
            Some(wire.clone()),
            Some(pin.node),
            Some(frame_id)
        ),
        Gesture::Pan { moved: true }
    );
    assert_eq!(
        gesture(
            0,
            "touch",
            Some(pin),
            Some(wire.clone()),
            Some(pin.node),
            Some(frame_id)
        ),
        Gesture::Wire(pin)
    );
    assert_eq!(
        gesture(
            0,
            "touch",
            None,
            Some(wire.clone()),
            Some(pin.node),
            Some(frame_id)
        ),
        Gesture::Select(wire)
    );
    assert_eq!(
        gesture(2, "mouse", Some(pin), None, Some(pin.node), Some(frame_id)),
        Gesture::Node(pin.node)
    );
    assert_eq!(
        gesture(0, "touch", None, None, Some(pin.node), Some(frame_id)),
        Gesture::Node(pin.node)
    );
    assert_eq!(
        gesture(0, "mouse", None, None, None, Some(frame_id)),
        Gesture::Frame(frame_id)
    );
    assert_eq!(gesture(2, "mouse", None, None, None, None), Gesture::Ignore);
    assert_eq!(
        gesture(0, "touch", None, None, None, None),
        Gesture::Pan { moved: false }
    );
    assert_eq!(gesture(0, "mouse", None, None, None, None), Gesture::Box);
}

#[test]
fn wire_attributes_validate_ids_and_pins_and_keep_unknown_kind_as_data() {
    let (from, to) = ends(false);
    let a = from.node.0.to_string();
    let b = to.node.0.to_string();
    let c = ConnectionModel {
        src_node: from.node,
        src_pin: PinId(1),
        dst_node: to.node,
        dst_pin: PinId(2),
    };
    assert_eq!(
        wire_selection(&a, "1", &b, "2", "exec"),
        Some(Selection::Exec(c))
    );
    assert_eq!(
        wire_selection(&a, "1", &b, "2", "unknown"),
        Some(Selection::Data(c))
    );
    assert_eq!(wire_selection("bad", "1", &b, "2", "exec"), None);
    assert_eq!(wire_selection(&a, "bad", &b, "2", "exec"), None);
    assert_eq!(wire_selection(&a, "1", "bad", "2", "exec"), None);
    assert_eq!(wire_selection(&a, "1", &b, "-1", "exec"), None);
}

#[test]
fn wire_drags_send_identical_commands_from_either_end() {
    for exec in [false, true] {
        let (from, to) = ends(exec);
        let c = ConnectionModel {
            src_node: from.node,
            src_pin: from.pin,
            dst_node: to.node,
            dst_pin: to.pin,
        };
        let expected = if exec {
            ClientMessage::ConnectExec(c)
        } else {
            ClientMessage::ConnectData(c)
        };
        for (a, b) in [(from, to), (to, from)] {
            assert_eq!(wire_command(a, b).unwrap(), Some(expected.clone()));
        }
        assert_eq!(wire_command(from, from), Ok(None));
        assert_eq!(
            wire_command(
                from,
                WireEnd {
                    dir: from.dir,
                    ..to
                }
            ),
            Err("[ERROR] Only an output can be connected to an input")
        );
        assert_eq!(
            wire_command(from, WireEnd { exec: !exec, ..to }),
            Err("[ERROR] An exec pin can only be connected to an exec pin")
        );
    }
}

#[test]
fn route_gestures_prioritize_points_but_keep_node_and_pan_actions() {
    let (a, b) = ends(false);
    let wire = Selection::Data(ConnectionModel {
        src_node: a.node,
        src_pin: a.pin,
        dst_node: b.node,
        dst_pin: b.pin,
    });
    assert_eq!(
        route_gesture(0, None, Some(wire.clone()), Some(2)),
        Some(Gesture::Point {
            wire: wire.clone(),
            index: 2
        })
    );
    assert_eq!(
        route_gesture(2, None, Some(wire.clone()), Some(2)),
        Some(Gesture::Context {
            wire: wire.clone(),
            index: Some(2)
        })
    );
    assert_eq!(
        route_gesture(2, None, Some(wire.clone()), None),
        Some(Gesture::Context {
            wire: wire.clone(),
            index: None
        })
    );
    assert_eq!(
        route_gesture(2, Some(a.node), Some(wire.clone()), Some(2)),
        None
    );
    assert_eq!(route_gesture(1, None, Some(wire.clone()), Some(2)), None);
    assert_eq!(route_gesture(0, None, Some(wire), None), None);
    assert_eq!(route_gesture(2, None, None, None), None);
}

#[test]
fn point_edits_snap_insert_in_order_remove_clear_and_enforce_limit() {
    use super::super::Position;
    let (a, b) = ((0.0, 0.0), (400.0, 100.0));
    let points = [Position { x: 200.0, y: 0.0 }];
    let edited = edit_points(&points, a, b, (302.0, 50.0), PointEdit::Add).unwrap();
    assert_eq!(edited, [points[0], Position { x: 304.0, y: 52.0 }]);
    assert_eq!(
        edit_points(&edited, a, b, a, PointEdit::Remove(0)).unwrap(),
        [edited[1]]
    );
    assert_eq!(
        edit_points(&points, a, b, a, PointEdit::Clear),
        Some(vec![])
    );
    assert_eq!(edit_points(&[], a, b, a, PointEdit::Clear), None);
    assert_eq!(edit_points(&points, a, b, a, PointEdit::Remove(1)), None);
    let full = vec![Position::default(); ticknode_shared::MAX_WIRE_POINTS];
    assert_eq!(edit_points(&full, a, b, a, PointEdit::Add), None);
}

#[test]
fn group_drag_captures_only_routes_with_both_endpoints_selected() {
    use super::super::{Position, WireRoutes};
    let (a, b) = ends(false);
    let third = NodeId::generate();
    let inside = ConnectionModel {
        src_node: a.node,
        src_pin: a.pin,
        dst_node: b.node,
        dst_pin: b.pin,
    };
    let outside = ConnectionModel {
        dst_node: third,
        ..inside
    };
    let all = WireRoutes::from([
        ((true, inside), vec![Position::default()]),
        ((false, inside), vec![Position { x: 4.0, y: 8.0 }]),
        ((false, outside), vec![Position::default()]),
    ]);
    let selected = [(a.node, Position::default()), (b.node, Position::default())];
    let routes = group_routes(&all, &selected);
    assert_eq!(routes.len(), 2);
    assert!(routes.contains_key(&(true, inside)));
    assert!(routes.contains_key(&(false, inside)));
    assert!(group_routes(&all, &selected[..1]).is_empty());
}

#[test]
fn group_translation_snaps_the_dragged_card_even_if_its_original_position_is_off_grid() {
    use super::super::Position;
    assert_eq!(
        group_delta(Position { x: 10.0, y: -10.0 }, (0.0, 0.0)),
        Position { x: 2.0, y: -2.0 }
    );
    assert_eq!(
        group_delta(Position { x: 8.0, y: 12.0 }, (5.0, -3.0)),
        Position { x: 4.0, y: -4.0 }
    );
}
