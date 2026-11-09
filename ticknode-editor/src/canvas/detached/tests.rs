use super::*;

fn detached(exec: bool, end: DetachedEnd) -> DetachedConnection {
    DetachedConnection { exec, end, row: 3, connection: ConnectionModel {
        src_node: NodeId::generate(), src_pin: PinId(1), dst_node: NodeId::generate(), dst_pin: PinId(2),
    } }
}

#[test]
fn broken_end_determines_live_origin_and_output_input_order() {
    for exec in [false, true] {
        for end in [DetachedEnd::Source, DetachedEnd::Target] {
            let d = detached(exec, end);
            let (live, broken) = ends(d);
            let c = d.connection;
            assert_eq!(live.exec, exec);
            assert_eq!(broken.exec, exec);
            match end {
                DetachedEnd::Source => {
                    assert_eq!((live.node, live.pin, live.dir), (c.dst_node, c.dst_pin, PinDirection::Input));
                    assert_eq!((broken.node, broken.pin, broken.dir), (c.src_node, c.src_pin, PinDirection::Output));
                }
                DetachedEnd::Target => {
                    assert_eq!((live.node, live.pin, live.dir), (c.src_node, c.src_pin, PinDirection::Output));
                    assert_eq!((broken.node, broken.pin, broken.dir), (c.dst_node, c.dst_pin, PinDirection::Input));
                }
            }
            let to = WireEnd { node: NodeId::generate(), ..broken };
            let kind = if exec { PinType::Exec } else { PinType::Data(DataType::Number) };
            let command = reconnect(d, to, Some(&kind), Some(&kind), &TypeRegistry::new(), &[]).unwrap();
            let expected = if end == DetachedEnd::Target {
                ConnectionModel { dst_node: to.node, dst_pin: to.pin, ..c }
            } else { ConnectionModel { src_node: to.node, src_pin: to.pin, ..c } };
            assert_eq!(command, ClientMessage::Batch(vec![if exec { ClientMessage::ConnectExec(expected) } else { ClientMessage::ConnectData(expected) }, ClientMessage::RemoveDetached(d)]));
        }
    }
}

#[test]
fn reconnection_rejects_missing_wrong_direction_or_incompatible_pins() {
    let d = detached(false, DetachedEnd::Target);
    let (live, to) = ends(d);
    let number = PinType::Data(DataType::Number);
    let object = PinType::Data(DataType::Object);
    let types = TypeRegistry::new();
    assert!(reconnect(d, to, None, Some(&number), &types, &[]).is_none());
    assert!(reconnect(d, to, Some(&number), None, &types, &[]).is_none());
    assert!(reconnect(d, live, Some(&number), Some(&number), &types, &[]).is_none());
    assert!(reconnect(d, to, Some(&number), Some(&object), &types, &[]).is_none());
    assert!(reconnect(d, WireEnd { exec: true, ..to }, Some(&number), Some(&PinType::Exec), &types, &[]).is_none());
    assert!(reconnect(d, to, Some(&PinType::Exec), Some(&PinType::Exec), &types, &[]).is_none());
    let string = PinType::Data(DataType::String);
    assert!(reconnect(d, to, Some(&number), Some(&string), &types, &[]).is_some());
}

#[test]
fn data_cycles_cancel_without_removing_the_detached_wire() {
    let d = detached(false, DetachedEnd::Target);
    let (live, to) = ends(d);
    let kind = PinType::Data(DataType::Number);
    let types = TypeRegistry::new();
    let middle = NodeId::generate();
    let data = [
        ConnectionModel { src_node: to.node, src_pin: PinId(8), dst_node: middle, dst_pin: PinId(9) },
        ConnectionModel { src_node: middle, src_pin: PinId(8), dst_node: live.node, dst_pin: PinId(9) },
    ];
    assert!(reconnect(d, to, Some(&kind), Some(&kind), &types, &data).is_none());
    assert!(reconnect(d, WireEnd { node: live.node, ..to }, Some(&kind), Some(&kind), &types, &[]).is_none());
    assert!(reconnect(d, to, Some(&kind), Some(&kind), &types, &data[..1]).is_some());
    let exec = DetachedConnection { exec: true, ..d };
    assert!(reconnect(exec, WireEnd { exec: true, ..to }, Some(&PinType::Exec), Some(&PinType::Exec), &types, &data).is_some());
}

#[test]
fn reconnection_uses_the_live_pins_current_kind_when_both_ends_changed() {
    for old_exec in [false, true] {
        for end in [DetachedEnd::Source, DetachedEnd::Target] {
            let d = detached(old_exec, end);
            let (_, broken) = ends(d);
            let to = WireEnd { exec: !old_exec, ..broken };
            let kind = if to.exec { PinType::Exec } else { PinType::Data(DataType::Number) };
            let command = reconnect(d, to, Some(&kind), Some(&kind), &TypeRegistry::new(), &[]).unwrap();
            assert_eq!(command, ClientMessage::Batch(vec![if to.exec { ClientMessage::ConnectExec(d.connection) } else { ClientMessage::ConnectData(d.connection) }, ClientMessage::RemoveDetached(d)]));
        }
    }
}
