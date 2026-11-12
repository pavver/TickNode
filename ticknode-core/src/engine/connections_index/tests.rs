use super::*;

#[test]
fn add_and_remove_exec_keeps_lookups_consistent() {
    let mut index = ConnectionIndex::default();
    let n1 = NodeId::generate();
    let n2 = NodeId::generate();
    let n3 = NodeId::generate();
    let c1 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(0),
        dst_node: n2,
        dst_pin: PinId(0),
    };
    let c2 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(0),
        dst_node: n3,
        dst_pin: PinId(0),
    };

    index.add_exec(c1);
    index.add_exec(c2);

    assert_eq!(index.exec(), &[c1, c2]);
    assert!(index.contains_exec(&c1));
    assert!(index.contains_exec(c2));
    assert_eq!(index.exec_successors(n1, PinId(0)), &[c1, c2]);
    assert_eq!(index.exec_successors(n1, PinId(1)), &[]);

    // Duplicate add should be ignored
    index.add_exec(c1);
    assert_eq!(index.exec(), &[c1, c2]);
    assert_eq!(index.exec_successors(n1, PinId(0)), &[c1, c2]);

    // Remove first connection
    assert!(index.remove_exec(&c1));
    assert_eq!(index.exec(), &[c2]);
    assert!(!index.contains_exec(&c1));
    assert!(index.contains_exec(c2));
    assert_eq!(index.exec_successors(n1, PinId(0)), &[c2]);

    // Removing non-existent connection returns false
    assert!(!index.remove_exec(c1));

    // Remove remaining connection
    assert!(index.remove_exec(c2));
    assert_eq!(index.exec(), &[]);
    assert!(!index.contains_exec(c2));
    assert_eq!(index.exec_successors(n1, PinId(0)), &[]);
}

#[test]
fn set_data_replaces_old_source_and_keeps_lookups_consistent() {
    let mut index = ConnectionIndex::default();
    let src1 = NodeId::generate();
    let src2 = NodeId::generate();
    let dst = NodeId::generate();
    let pin_in1 = PinId(1);
    let pin_in2 = PinId(2);

    let c1 = ConnectionModel {
        src_node: src1,
        src_pin: PinId(0),
        dst_node: dst,
        dst_pin: pin_in1,
    };
    let c2 = ConnectionModel {
        src_node: src1,
        src_pin: PinId(1),
        dst_node: dst,
        dst_pin: pin_in2,
    };

    // Initially nothing connected
    assert_eq!(index.data(), &[]);
    assert_eq!(index.data_into(dst), &[]);
    assert_eq!(index.data_source(dst, pin_in1), None);

    // Connect first data connection
    assert_eq!(index.set_data(c1), None);
    assert_eq!(index.data(), &[c1]);
    assert!(index.contains_data(&c1));
    assert_eq!(index.data_into(dst), &[c1]);
    assert_eq!(index.data_source(dst, pin_in1), Some(c1));

    // Connect second pin on the same destination node
    assert_eq!(index.set_data(c2), None);
    assert_eq!(index.data(), &[c1, c2]);
    assert_eq!(index.data_into(dst), &[c1, c2]);
    assert_eq!(index.data_source(dst, pin_in2), Some(c2));

    // Replace source for pin_in1 with src2
    let c1_replaced = ConnectionModel {
        src_node: src2,
        src_pin: PinId(5),
        dst_node: dst,
        dst_pin: pin_in1,
    };
    assert_eq!(index.set_data(c1_replaced), Some(c1));

    // Order semantics: old c1 removed, c1_replaced appended
    assert_eq!(index.data(), &[c2, c1_replaced]);
    assert!(!index.contains_data(&c1));
    assert!(index.contains_data(&c1_replaced));
    assert_eq!(index.data_into(dst), &[c2, c1_replaced]);
    assert_eq!(index.data_source(dst, pin_in1), Some(c1_replaced));
    assert_eq!(index.data_source(dst, pin_in2), Some(c2));

    // Remove data input
    assert_eq!(index.remove_data_input(dst, pin_in1), Some(c1_replaced));
    assert_eq!(index.data(), &[c2]);
    assert!(!index.contains_data(&c1_replaced));
    assert_eq!(index.data_into(dst), &[c2]);
    assert_eq!(index.data_source(dst, pin_in1), None);

    // Removing again returns None
    assert_eq!(index.remove_data_input(dst, pin_in1), None);

    // Remove remaining input
    assert_eq!(index.remove_data_input(dst, pin_in2), Some(c2));
    assert_eq!(index.data(), &[]);
    assert_eq!(index.data_into(dst), &[]);
    assert_eq!(index.data_source(dst, pin_in2), None);
}

#[test]
fn retain_filters_and_rebuilds_indices() {
    let mut index = ConnectionIndex::default();
    let n1 = NodeId::generate();
    let n2 = NodeId::generate();
    let n3 = NodeId::generate();

    let e1 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(0),
        dst_node: n2,
        dst_pin: PinId(0),
    };
    let e2 = ConnectionModel {
        src_node: n2,
        src_pin: PinId(0),
        dst_node: n3,
        dst_pin: PinId(0),
    };
    let e3 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(1),
        dst_node: n3,
        dst_pin: PinId(1),
    };

    let d1 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(2),
        dst_node: n2,
        dst_pin: PinId(1),
    };
    let d2 = ConnectionModel {
        src_node: n2,
        src_pin: PinId(2),
        dst_node: n3,
        dst_pin: PinId(1),
    };
    let d3 = ConnectionModel {
        src_node: n1,
        src_pin: PinId(3),
        dst_node: n3,
        dst_pin: PinId(2),
    };

    index.add_exec(e1);
    index.add_exec(e2);
    index.add_exec(e3);

    index.set_data(d1);
    index.set_data(d2);
    index.set_data(d3);

    // Drop any connections touching n2
    let removed = index.retain(
        |c| c.src_node != n2 && c.dst_node != n2,
        |c| c.src_node != n2 && c.dst_node != n2,
    );

    assert_eq!(removed.exec, vec![e1, e2]);
    assert_eq!(removed.data, vec![d1, d2]);

    assert_eq!(index.exec(), &[e3]);
    assert_eq!(index.data(), &[d3]);

    // Lookups no longer find removed connections
    assert!(!index.contains_exec(e1));
    assert!(!index.contains_exec(e2));
    assert!(index.contains_exec(e3));
    assert_eq!(index.exec_successors(n1, PinId(0)), &[]);
    assert_eq!(index.exec_successors(n1, PinId(1)), &[e3]);

    assert!(!index.contains_data(d1));
    assert!(!index.contains_data(d2));
    assert!(index.contains_data(d3));
    assert_eq!(index.data_into(n2), &[]);
    assert_eq!(index.data_into(n3), &[d3]);
    assert_eq!(index.data_source(n2, PinId(1)), None);
    assert_eq!(index.data_source(n3, PinId(2)), Some(d3));
}
