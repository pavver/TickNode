use super::*;

fn run(kind: &str, values: Vec<DataValue>) -> Vec<Output> {
    let mut node = StringPlugin::create(kind, NodeId::generate()).unwrap();
    assert_eq!(node.definition().type_name, kind);
    let base = Op::from_type(kind).unwrap().base();
    let inputs = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (PinId(base + 2 + i as u32), v))
        .collect();
    assert!(
        node.execute(Trigger::Pin(PinId(base + 1)), &inputs)
            .is_empty()
    );
    let mut outputs = node.execute(Trigger::Pin(PinId(base)), &inputs);
    assert_eq!(outputs.pop(), Some(Output::Exec(PinId(base + 1))));
    outputs
}
fn strings(values: &[&str]) -> DataValue {
    DataValue::List {
        element_type: DataType::String,
        elements: values.iter().map(|v| DataValue::from(*v)).collect(),
    }
}
#[test]
fn operations_preserve_unicode_empty_inputs_and_output_pins() {
    assert_eq!(
        run("string/concat", vec!["ї".into(), "жак".into()]),
        vec![Output::Data(PinId(4), "їжак".into())]
    );
    assert_eq!(
        run("string/length", vec!["їж🦀".into()]),
        vec![Output::Data(PinId(13), 3i64.into())]
    );
    assert_eq!(
        run(
            "string/substring",
            vec!["aї🦀z".into(), 1i64.into(), 2i64.into()]
        ),
        vec![Output::Data(PinId(25), "ї🦀".into())]
    );
    assert_eq!(
        run(
            "string/substring",
            vec!["abc".into(), (-3i64).into(), 9i64.into()]
        ),
        vec![Output::Data(PinId(25), "abc".into())]
    );
    assert_eq!(
        run("string/contains", vec!["abc".into(), "b".into()]),
        vec![Output::Data(PinId(34), true.into())]
    );
    assert_eq!(
        run("string/contains", vec![]),
        vec![Output::Data(PinId(34), true.into())]
    );
    assert_eq!(
        run("string/split", vec!["a,,b,".into(), ",".into()]),
        vec![Output::Data(PinId(44), strings(&["a", "", "b", ""]))]
    );
    assert_eq!(
        run("string/split", vec!["ї🦀".into()]),
        vec![Output::Data(PinId(44), strings(&["ї", "🦀"]))]
    );
    assert_eq!(
        run(
            "string/aggregate",
            vec![
                DataValue::List {
                    element_type: DataType::Any,
                    elements: vec!["a".into(), 7i64.into(), "b".into()]
                },
                "/".into()
            ]
        ),
        vec![Output::Data(PinId(64), "a/b".into())]
    );
    assert_eq!(
        run("string/aggregate", vec![false.into()]),
        vec![Output::Data(PinId(64), "".into())]
    );
    assert!(StringPlugin::create("unknown", NodeId::generate()).is_none());
}
#[test]
fn regex_returns_first_group_or_whole_match_and_handles_invalid_patterns() {
    for (pattern, matched, capture) in [
        ("a(b)", true, "b"),
        ("ab", true, "ab"),
        ("z", false, ""),
        ("[", false, ""),
        ("(z)?ab", true, "ab"),
    ] {
        assert_eq!(
            run("string/regex", vec!["abc".into(), pattern.into()]),
            vec![
                Output::Data(PinId(54), matched.into()),
                Output::Data(PinId(55), capture.into())
            ]
        );
    }
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

    for type_name in StringPlugin::NODE_TYPES {
        let node = StringPlugin::create(type_name, NodeId::generate()).unwrap();
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
