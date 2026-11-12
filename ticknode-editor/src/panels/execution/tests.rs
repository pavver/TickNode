use super::*;
use crate::i18n::{Locale, with_locale};

#[test]
fn execution_rows_distinguish_missing_values_connections_and_fired_pins() {
    with_locale(Locale::En, || {
        let pin = PinId(1);
        let mut trace = NodeTrace {
            node_id: NodeId::generate(),
            tick: 42,
            inputs: vec![],
            outputs: vec![],
            fired: vec![pin],
        };
        let view = |trace: Option<&NodeTrace>, dir, exec, connected| {
            trace_value(trace, dir, exec, pin, connected)
        };
        assert_eq!(
            view(None, PinDirection::Input, false, false),
            ("—".into(), true, t!("exec-node-never-hint"))
        );
        assert_eq!(
            view(Some(&trace), PinDirection::Input, true, false),
            (String::new(), true, String::new())
        );
        assert_eq!(
            view(Some(&trace), PinDirection::Output, true, false),
            ("▶ fired".into(), false, String::new())
        );
        trace.fired.clear();
        assert_eq!(
            view(Some(&trace), PinDirection::Output, true, true),
            ("—".into(), true, t!("exec-not-fired-hint"))
        );
        assert_eq!(
            view(Some(&trace), PinDirection::Input, false, true),
            ("—".into(), true, t!("exec-connected-after-hint"))
        );
        assert_eq!(
            view(Some(&trace), PinDirection::Input, false, false),
            (t!("exec-not-connected"), true, t!("exec-default-used-hint"))
        );
        assert_eq!(
            view(Some(&trace), PinDirection::Output, false, true),
            ("—".into(), true, t!("exec-output-not-emitted-hint"))
        );
        trace.inputs.push((pin, None));
        assert_eq!(
            view(Some(&trace), PinDirection::Input, false, true),
            (t!("exec-no-value"), true, t!("exec-no-value-hint"))
        );
        trace.inputs[0].1 = Some(DataValue::String("input".into()));
        assert_eq!(
            view(Some(&trace), PinDirection::Input, false, true),
            ("\"input\"".into(), false, "\"input\"".into())
        );
        trace.outputs.push((pin, DataValue::Bool(false)));
        assert_eq!(
            view(Some(&trace), PinDirection::Output, false, false),
            ("false".into(), false, "false".into())
        );
    });
}

#[test]
fn execution_values_limit_the_row_and_tooltip_separately() {
    let (row, muted, tooltip) = shown(&DataValue::String("ї".repeat(6000)));
    assert!(!muted);
    assert_eq!(row.chars().count(), MAX_VALUE_CHARS + 1);
    assert!(row.starts_with('"') && row.ends_with('…'));
    assert_eq!(tooltip.chars().count(), 5001);
}
