use super::*;
use crate::value_catalog::ValueCatalogIndex;
use std::sync::Arc;

fn add(e: &mut TickEngine, ty: &str, props: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    e.create_node(id, ty, props).unwrap();
    id
}
fn render(e: &mut TickEngine, id: NodeId) -> DataValue {
    e.pull_data(id, PinId(0), &mut vec![]).unwrap()
}
fn object(key: &str, value: DataValue, ty: DataType) -> DataValue {
    DataValue::Object {
        type_name: None,
        fields: [(
            key.into(),
            ObjectField {
                value,
                data_type: Some(ty),
            },
        )]
        .into(),
    }
}
fn setup() -> TickEngine {
    let mut e = TickEngine::new();
    e.plugin_values.catalogs.insert(
        "ha".into(),
        Arc::new(ValueCatalogIndex::new(vec![ValueEntry {
            key: "sensor.one".into(),
            title: "One".into(),
            icon: None,
            fields: vec![ValueField {
                path: "state".into(),
                title: "State".into(),
                data_type: DataType::Named("quantity.power".into()),
            }],
        }])),
    );
    e
}
fn input(e: &TickEngine, id: NodeId, name: &str) -> PinId {
    e.node_definition(id)
        .unwrap()
        .pins
        .iter()
        .find(|p| p.name == name)
        .unwrap()
        .id
}

#[test]
fn quantities_format_identically_through_wire_both_bindings_and_direct_references_in_both_locales()
{
    let mut e = setup();
    let power = DataType::Named("quantity.power".into());
    e.variables
        .create(VariableKey::global("x"), power.clone(), 1200i64.into());
    let template = add(&mut e, "text/template", &[("template".into(), "{{ text.wire }}|{{ text.bound }}|{{ text.plugin }}|{{ text.var.x }}|{{ text.ha.sensor.one.state }}".into())]);
    let source = add(
        &mut e,
        "const",
        &[
            ("value_type".into(), "quantity.power".into()),
            ("value".into(), 1200i64.into()),
        ],
    );
    e.connect_data(source, PinId(0), template, input(&e, template, "wire"))
        .unwrap();
    e.set_input_variable(
        template,
        input(&e, template, "bound"),
        Some(VariableRef::parse("var.x").unwrap()),
    )
    .unwrap();
    e.set_input_variable(
        template,
        input(&e, template, "plugin"),
        Some(VariableRef::parse("ha.sensor.one.state").unwrap()),
    )
    .unwrap();
    e.plugin_values.values.insert(
        "ha.sensor.one".into(),
        object("state", 1200i64.into(), power),
    );
    for (locale, shown) in [("en", "1.2 kW"), ("uk", "1,2 кВт")] {
        e.set_locale(locale);
        assert_eq!(render(&mut e, template), [shown; 5].join("|").into());
    }
}

#[test]
fn uncataloged_nested_fields_keep_units_in_templates_to_text_match_and_converting_bindings() {
    let mut e = setup();
    let temperature = DataType::Named("quantity.temperature".into());
    let value = object(
        "attributes",
        object("temperature", 20.5.into(), temperature.clone()),
        DataType::Object,
    );
    for target in [DataType::Any, DataType::Object] {
        let stored = fit_typed_variable_value(
            &e.types,
            value.clone(),
            &DataType::Object,
            &target,
            &Default::default(),
            crate::quantity::labels("en"),
        )
        .unwrap();
        e.variables.delete(&VariableKey::global("tree"));
        e.variables
            .create(VariableKey::global("tree"), target, stored);
        e.variables.set(&VariableKey::global("tree"), value.clone());
    }
    let template = add(&mut e, "text/template", &[("template".into(), "{{ text.value.attributes.temperature }}|{{ text.var.tree.attributes.temperature }}|{{ text.ha.sensor.one.attributes.temperature }}".into())]);
    e.set_input_variable(
        template,
        input(&e, template, "value"),
        Some(VariableRef::parse("var.tree").unwrap()),
    )
    .unwrap();
    e.plugin_values
        .values
        .insert("ha.sensor.one".into(), value.clone());
    let format = add(&mut e, "text/to_text", &[]);
    e.set_input_variable(
        format,
        PinId(1),
        Some(VariableRef::parse("ha.sensor.one.attributes").unwrap()),
    )
    .unwrap();
    let text_sink = add(
        &mut e,
        "text/template",
        &[("template".into(), "{{ value }}".into())],
    );
    e.set_input_variable(
        text_sink,
        input(&e, text_sink, "value"),
        Some(VariableRef::parse("ha.sensor.one.attributes.temperature").unwrap()),
    )
    .unwrap();
    assert_eq!(
        e.plugin_values
            .resolve("ha.sensor.one.attributes.temperature")
            .unwrap()
            .1,
        temperature
    );
    for (locale, shown) in [("en", "20.5 °C"), ("uk", "20,5 °С")] {
        e.set_locale(locale);
        assert_eq!(render(&mut e, template), [shown; 3].join("|").into());
        assert_eq!(
            render(&mut e, format),
            format!("{{temperature: {shown}}}").into()
        );
    }
    let case = condition::MatchCase::parse("1: var.tree.attributes.temperature > 20 °C").unwrap();
    assert!(case.matches_in(
        &e.types,
        |name| {
            let r = VariableRef::parse(name).unwrap();
            e.variables
                .resolve_value(&r.name, r.lookup, None, &e.types)
                .unwrap()
                .1
        },
        |name| {
            let r = VariableRef::parse(name).unwrap();
            e.variables
                .resolve_value(&r.name, r.lookup, None, &e.types)
                .map(|(v, _)| v.clone())
        }
    ));
    let formula = add(
        &mut e,
        "math/expression",
        &[(
            "expression".into(),
            "var.tree.attributes.temperature + ha.sensor.one.attributes.temperature".into(),
        )],
    );
    assert_eq!(render(&mut e, formula), 41i64.into());
    let result = add(
        &mut e,
        "text/template",
        &[("template".into(), "{{ text.value }}".into())],
    );
    e.connect_data(formula, PinId(0), result, PinId(1)).unwrap();
    assert_eq!(render(&mut e, result), "41 °С".into());
}
