use super::*;

#[test]
fn trace_lookup_distinguishes_directions_and_missing_values() {
    let trace = NodeTrace {
        node_id: NodeId::generate(),
        tick: 7,
        inputs: vec![(PinId(1), Some(12i64.into())), (PinId(2), None)],
        outputs: vec![(PinId(1), "output".into())],
        fired: vec![PinId(3)],
    };
    assert_eq!(
        trace_value(&trace, PinId(1), PinDirection::Input),
        Some(&12i64.into())
    );
    assert_eq!(
        trace_value(&trace, PinId(1), PinDirection::Output),
        Some(&"output".into())
    );
    assert_eq!(trace_value(&trace, PinId(2), PinDirection::Input), None);
    assert_eq!(trace_value(&trace, PinId(99), PinDirection::Output), None);
}

#[test]
fn truncation_keeps_unicode_characters_whole() {
    assert_eq!(truncate("привіт".into(), 3), "при…");
    assert_eq!(truncate("abc".into(), 3), "abc");
    assert_eq!(truncate("😀😀😀".into(), 2), "😀😀…");
}
