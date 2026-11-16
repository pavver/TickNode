use crate::{
    DataType, DataValue, ObjectField,
    template::{CompiledTemplate, RenderEnv, variables_with_namespaces},
};
use std::sync::Arc;
fn object(fields: impl IntoIterator<Item = (&'static str, DataValue)>) -> DataValue {
    DataValue::Object {
        type_name: None,
        fields: fields
            .into_iter()
            .map(|(name, value)| {
                (
                    name.into(),
                    ObjectField {
                        name: name.into(),
                        description: String::new(),
                        value,
                    },
                )
            })
            .collect(),
    }
}
#[test]
fn namespace_global_and_text_fields_preserve_types_without_input_pins() {
    let source = "{{ ha.sensor.power.state * 2 }} / {{ text.ha.sensor.power.state }} / {{ ha.sensor.power.attributes.enabled }} / {{ input }}";
    assert_eq!(
        variables_with_namespaces(source, &["ha".into()]).unwrap(),
        vec!["input"]
    );
    let value = object([
        ("state", 1200i64.into()),
        ("attributes", object([("enabled", true.into())])),
    ]);
    let env = RenderEnv {
        namespaces: vec!["ha".into()],
        plugin_values: vec![
            ("ha.sensor.power".into(), Some(value), DataType::Object),
            (
                "ha.sensor.power.state".into(),
                Some(1200i64.into()),
                DataType::Named("quantity.power".into()),
            ),
        ],
        ..RenderEnv::default()
    };
    let format = |value: &DataValue, ty: &DataType| {
        crate::to_text::to_text(
            value,
            ty,
            &Default::default(),
            &Default::default(),
            &crate::quantity::CanonicalLabels,
        )
    };
    let text = CompiledTemplate::new(source)
        .unwrap()
        .render_localized(
            &[("input".into(), Some("ok".into()), DataType::String)],
            &format,
            Arc::new(crate::text::format_value),
            env,
            &crate::TypeRegistry::new(),
        )
        .unwrap();
    assert!(text.starts_with("2400 / 1.2 kW / True / ok"), "{text}");
}
#[test]
fn static_paths_exclude_local_bindings_and_dynamic_entries() {
    let compiled = CompiledTemplate::new(
        "{% set local = 1 %}{{ local }} {{ ha.sensor.one.state }} {{ text.ha.sensor.two.state }}",
    )
    .unwrap();
    let paths = compiled.static_paths();
    assert!(paths.contains(&"ha.sensor.one.state".into()));
    assert!(paths.contains(&"text.ha.sensor.two.state".into()));
    assert!(!paths.iter().any(|p| p.starts_with("local")));
}
