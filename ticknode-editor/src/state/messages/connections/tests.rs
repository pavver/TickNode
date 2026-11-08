use super::*;

#[test]
fn repeated_events_preserve_exec_order_and_replace_only_the_data_destination() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let c = NodeId::generate();
    let first = ConnectionModel { src_node: a, src_pin: PinId(1), dst_node: b, dst_pin: PinId(2) };
    let other_pin = ConnectionModel { dst_pin: PinId(3), ..first };
    let other_node = ConnectionModel { dst_node: c, ..first };
    let replacement = ConnectionModel { src_node: c, ..first };
    let mut exec = vec![first, other_pin];
    connect_exec(&mut exec, first);
    assert_eq!(exec, [first, other_pin]);
    connect_exec(&mut exec, replacement);
    assert_eq!(exec, [first, other_pin, replacement]);

    let mut data = vec![first, other_pin, other_node];
    connect_data(&mut data, replacement);
    connect_data(&mut data, replacement);
    assert_eq!(data, [other_pin, other_node, replacement]);
    disconnect_data(&mut data, b, PinId(2));
    disconnect_data(&mut data, b, PinId(2));
    assert_eq!(data, [other_pin, other_node]);
}
