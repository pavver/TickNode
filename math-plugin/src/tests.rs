use super::*;

#[test]
fn every_operation_uses_its_declared_pins_and_normalizes_results() {
    let cases = [
        ("math/sin", 0, 0.0, 0.0, 0.0),
        ("math/cos", 10, 0.0, 0.0, 1.0),
        ("math/abs", 20, -3.5, 0.0, 3.5),
        ("math/pow", 30, 2.0, 3.0, 8.0),
        ("math/sqrt", 40, 9.0, 0.0, 3.0),
        ("math/round", 50, -2.5, 0.0, -3.0),
        ("math/min", 60, 4.0, -2.0, -2.0),
        ("math/max", 70, 4.0, -2.0, 4.0),
    ];
    for (kind, base, a, b, expected) in cases {
        let mut node = MathPlugin::create(kind, NodeId::generate()).unwrap();
        let def = node.definition();
        assert_eq!(def.type_name, kind);
        let result_pin = def.pins.last().unwrap().id;
        let inputs = [(PinId(base + 2), a.into()), (PinId(base + 3), b.into())]
            .into_iter()
            .collect();
        assert_eq!(
            node.execute(Trigger::Pin(PinId(base)), &inputs),
            vec![
                Output::Data(result_pin, DataValue::from(expected)),
                Output::Exec(PinId(base + 1)),
            ]
        );
        assert!(
            node.execute(Trigger::Pin(PinId(base + 1)), &inputs)
                .is_empty()
        );
        assert_eq!(
            node.execute(Trigger::Pin(PinId(base)), &Inputs::default())[0],
            Output::Data(
                result_pin,
                DataValue::from(match kind {
                    "math/cos" | "math/pow" => 1.0,
                    _ => 0.0,
                })
            )
        );
    }
    assert!(MathPlugin::create("math/unknown", NodeId::generate()).is_none());
}

#[test]
fn invalid_square_root_remains_nan() {
    let inputs = [(PinId(42), DataValue::from(-1.0))].into_iter().collect();
    let output = MathNode(Op::Sqrt).execute(Trigger::Pin(PinId(40)), &inputs);
    assert!(
        matches!(output[0], Output::Data(PinId(43), DataValue::Number(NumberValue::Float(n))) if n.is_nan())
    );
}

#[test]
fn ukrainian_translations_cover_every_node_and_pin() {
    let uk = include_str!("../locales/uk.ftl");
    let keys: std::collections::HashSet<&str> = uk
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                None
            } else {
                line.split('=').next().map(|k| k.trim())
            }
        })
        .collect();

    for type_name in MathPlugin::NODE_TYPES {
        let node = MathPlugin::create(type_name, NodeId::generate()).unwrap();
        let def = node.definition();
        let slug = def.type_name.replace('/', "-");
        assert!(
            keys.contains(&format!("node-{slug}-title").as_str()),
            "missing title for {type_name}"
        );
        assert!(
            keys.contains(&format!("node-{slug}-description").as_str()),
            "missing description for {type_name}"
        );
        assert!(
            keys.contains(&format!("node-{slug}-help").as_str()),
            "missing help for {type_name}"
        );
        for pin in &def.pins {
            assert!(
                keys.contains(&format!("node-{slug}-pin-{}", pin.name).as_str()),
                "missing pin translation for {type_name} pin {}",
                pin.name
            );
        }
    }
}
