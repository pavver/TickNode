use super::*;
use crate::{EngineError, TickEngine};

pub(super) fn setup(source: &str) -> (TickEngine, NodeId) {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node_interactive(id, "text/template", &[("template".into(), source.into())])
        .unwrap();
    (engine, id)
}

pub(super) fn pins(engine: &TickEngine, id: NodeId) -> Vec<(String, PinId)> {
    engine
        .node_definition(id)
        .unwrap()
        .pins
        .into_iter()
        .filter(|p| p.direction == PinDirection::Input)
        .map(|p| (p.name, p.id))
        .collect()
}

pub(super) fn inline(engine: &mut TickEngine, id: NodeId, name: &str, value: DataValue) {
    let pin = pins(engine, id)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1;
    engine.set_input_value(id, pin, Some(value)).unwrap();
}

pub(super) fn connect_value(engine: &mut TickEngine, id: NodeId, name: &str, value: DataValue) {
    let source = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), value)])
        .unwrap();
    let pin = pins(engine, id)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1;
    engine.connect_data(source, PinId(0), id, pin).unwrap();
}

pub(super) fn render(engine: &mut TickEngine, id: NodeId) -> Option<DataValue> {
    let output = engine.pull_data(id, PinId(0), &mut vec![]);
    engine.take_node_error_changes();
    output
}

#[test]
fn variables_follow_first_appearance_and_respect_jinja_scopes_and_tokens() {
    let (engine, id) = setup(
        "z and a are plain text {# ignored #}{{ obj.a }}{{ z }}{{ a }}\
        {{ 'fake' }}{% for item in items %}{{ item }}{% endfor %}\
        {% set local = other %}{{ local }}{{ range(2) }}{{ z }}",
    );
    assert_eq!(
        pins(&engine, id),
        vec![
            ("obj".into(), PinId(1)),
            ("z".into(), PinId(2)),
            ("a".into(), PinId(3)),
            ("items".into(), PinId(4)),
            ("other".into(), PinId(5)),
        ]
    );
    assert!(
        engine
            .node_definition(id)
            .unwrap()
            .pins
            .iter()
            .filter(|p| p.id != PinId(0))
            .all(|p| !p.required && p.pin_type == PinType::Data(DataType::Any))
    );
}

#[test]
fn ids_survive_reorder_reload_and_property_order_but_removed_ids_never_return() {
    let (mut engine, id) = setup("{{ b }}{{ a }}");
    inline(&mut engine, id, "a", "kept".into());
    engine
        .set_node_property(id, "template", "{{ a }}{{ c }}".into())
        .unwrap();
    assert_eq!(
        pins(&engine, id),
        vec![("a".into(), PinId(2)), ("c".into(), PinId(3))]
    );
    let mut properties = engine.node_model(id).unwrap().properties;
    properties.reverse();
    let mut copy = TickEngine::new();
    copy.create_node(id, "text/template", &properties).unwrap();
    assert_eq!(render(&mut copy, id), Some("kept".into()));
    copy.set_node_property(id, "template", "{{ c }}{{ a }}{{ b }}".into())
        .unwrap();
    assert_eq!(
        pins(&copy, id),
        vec![
            ("c".into(), PinId(3)),
            ("a".into(), PinId(2)),
            ("b".into(), PinId(4))
        ]
    );
    copy.set_node_property(id, "template", "empty".into())
        .unwrap();
    let graph = copy.export_graph();
    let mut loaded = TickEngine::new();
    loaded.load_graph(&graph);
    loaded
        .set_node_property(id, "template", "{{ a }}".into())
        .unwrap();
    assert_eq!(pins(&loaded, id), vec![("a".into(), PinId(5))]);
}

#[test]
fn edits_keep_surviving_wires_and_prune_deleted_wires_and_inline_values() {
    let (mut engine, id) = setup("{{ a }}{{ b }}");
    let source = NodeId::generate();
    engine.create_node(source, "const", &[]).unwrap();
    engine.connect_data(source, PinId(0), id, PinId(1)).unwrap();
    inline(&mut engine, id, "b", "removed".into());
    let removed = engine
        .set_node_property(id, "template", "{{ c }}{{ a }}".into())
        .unwrap();
    assert!(removed.data.is_empty());
    assert!(
        !engine
            .node_model(id)
            .unwrap()
            .properties
            .iter()
            .any(|(n, _)| n == "in:2")
    );
    let removed = engine
        .set_node_property(id, "template", "{{ c }}".into())
        .unwrap();
    assert_eq!(removed.data.len(), 1);
    assert_eq!(render(&mut engine, id), Some("".into()));
}

#[test]
fn plain_conversion_covers_every_data_value_kind() {
    let parse = |text, ty| text::parse_value(text, &ty).unwrap();
    let cases = vec![
        (
            DataValue::Number(NumberValue::Int(i64::MAX)),
            i64::MAX.to_string(),
        ),
        (DataValue::Number(NumberValue::Float(1.25)), "1.25".into()),
        (true.into(), "True".into()),
        (false.into(), "False".into()),
        ("<hello>".into(), "<hello>".into()),
        (parse("12:34:56", DataType::Time), "12:34:56".into()),
        (
            parse("2025-01-02T03:04:05Z", DataType::DateTime),
            "2025-01-02T03:04:05+00:00".into(),
        ),
        (parse("1h 30m", DataType::Duration), "1h 30m".into()),
        (DataValue::Ticks(42), "42t".into()),
        (parse("#12345678", DataType::Color), "#12345678".into()),
    ];
    for (value, expected) in cases {
        let (mut engine, id) = setup("{{ x }}");
        inline(&mut engine, id, "x", value.clone());
        assert_eq!(render(&mut engine, id), Some(expected.into()), "{value:?}");
    }
    assert!(values::plain(&DataValue::Number(NumberValue::Int(2))).is_integer());
    assert!(!values::plain(&DataValue::Number(NumberValue::Float(2.0))).is_integer());
}

#[test]
fn lists_structs_conditions_missing_values_and_arithmetic() {
    let (mut engine, id) = setup(
        "{% for item in items %}{{ item }},{% endfor %}\
        {{ obj.name }}:{{ obj.count + 2 }}|{% if on %}on{% else %}off{% endif %}|\
        {{ missing | default('—') }}|{{ missing | text }}|{{ text.missing | default('unset') }}|{{ missing is undefined }}|{{ text.missing is undefined }}|\
        {{ obj is mapping }}|{{ obj == {'name': 'sensor', 'count': 40} }}",
    );
    connect_value(
        &mut engine,
        id,
        "items",
        DataValue::List {
            element_type: DataType::Number,
            elements: vec![1.into(), 2.into()],
        },
    );
    let fields = [("name", "sensor".into()), ("count", 40.into())]
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
        .collect();
    connect_value(
        &mut engine,
        id,
        "obj",
        DataValue::Object {
            fields,
            type_name: None,
        },
    );
    inline(&mut engine, id, "on", false.into());
    assert_eq!(
        render(&mut engine, id),
        Some("1,2,sensor:42|off|—||unset|True|True|True|True".into())
    );
    inline(&mut engine, id, "on", true.into());
    assert_eq!(
        render(&mut engine, id),
        Some("1,2,sensor:42|on|—||unset|True|True|True|True".into())
    );
}

#[test]
fn syntax_rejection_is_translated_and_atomic_for_edits_and_creation() {
    let (mut engine, id) = setup("{{ x }}");
    let before = engine.export_graph();
    let error = engine
        .set_node_property(id, "template", "{% if %}".into())
        .unwrap_err();
    let EngineError::InvalidProperty(message) = error else {
        panic!("{error:?}")
    };
    assert_eq!(message.key, "error-template-syntax");
    assert!(!message.args.is_empty());
    assert_eq!(engine.export_graph(), before);
    let other = NodeId::generate();
    assert!(
        engine
            .create_node_interactive(other, "text/template", &[("template".into(), "{{".into())])
            .is_err()
    );
    assert!(!engine.has_node(other));
}

#[test]
fn fuel_exhaustion_reports_a_structured_error_and_logs_once_until_recovery() {
    let (mut engine, id) = setup("{% for i in range(100000) %}{{ i }}{% endfor %}");
    assert_eq!(render(&mut engine, id), None);
    let error = &engine.node_errors()[0];
    assert_eq!(error.node, id);
    assert_eq!(error.message.key, "error-template-render");
    assert!(error.message.fallback.contains("fuel"));
    assert_eq!(engine.host_state.logs.len(), 1);
    engine.tick();
    assert_eq!(render(&mut engine, id), None);
    assert_eq!(engine.node_errors()[0].count, 2);
    assert_eq!(engine.host_state.logs.len(), 1);
    engine
        .set_node_property(id, "template", "ok".into())
        .unwrap();
    assert_eq!(render(&mut engine, id), Some("ok".into()));
    assert!(engine.node_errors().is_empty());
    engine
        .set_node_property(id, "template", "{{ 1 // 0 }}".into())
        .unwrap();
    assert_eq!(render(&mut engine, id), None);
    assert_eq!(engine.host_state.logs.len(), 2);
}

#[test]
fn template_definition_properties_output_and_errors_have_both_translations() {
    let definition = TemplateNode::new(NodeId::generate()).definition();
    for (_, source) in crate::translations::CORE_TRANSLATIONS {
        let ids = crate::translations::message_ids(source);
        for suffix in [
            "title",
            "description",
            "help",
            "pin-Text",
            "prop-template-title",
            "prop-template-description",
        ] {
            assert!(ids.contains(&format!("node-text-template-{suffix}")));
        }
        for key in ["error-template-syntax", "error-template-render"] {
            assert!(ids.contains(&key.into()));
        }
        for property in definition
            .properties
            .iter()
            .filter(|p| p.kind != PropertyKind::Hidden)
        {
            assert!(ids.contains(&format!("node-text-template-prop-{}-title", property.name)));
        }
    }
}

#[test]
fn plain_inputs_support_standard_arithmetic_comparisons_tests_and_filters() {
    let (mut engine, id) = setup(
        "{{ x + 1 }}|{% if x > 5 %}high{% endif %}|{{ x == 6 }}|\
        {% if state == 'on' %}on{% endif %}|{{ state != 'off' }}|\
        {{ x in items }}|{{ items | sort | join(',') }}|{{ items | sum }}|\
        {{ (x / 4) | round(1) }}|{{ x is number }}|{{ x is integer }}|\
        {{ state is string }}|{{ enabled is boolean }}|{{ enabled == true }}|\
        {{ items == [6, 1, 3] }}|{{ items is sequence }}|\
        {{ items | map('int') | sum }}|{{ none is none }}",
    );
    inline(&mut engine, id, "x", 6.into());
    inline(&mut engine, id, "state", "on".into());
    inline(&mut engine, id, "enabled", true.into());
    connect_value(
        &mut engine,
        id,
        "items",
        DataValue::List {
            element_type: DataType::Number,
            elements: vec![6.into(), 1.into(), 3.into()],
        },
    );
    assert_eq!(
        render(&mut engine, id),
        Some(
            "7|high|True|on|True|True|1,3,6|10|1.5|True|True|True|True|True|True|True|10|True"
                .into()
        )
    );
}

#[test]
fn presentation_references_derive_inputs_in_order_without_a_text_input() {
    let (mut engine, id) = setup(
        "{{ text is mapping }}{{ text.power }}{{ obj.text }}{{ power }}\
        {{ text.name }}{{ name }}{{ 'literal' | text }}{{ range(1) }}",
    );
    assert_eq!(
        pins(&engine, id),
        vec![
            ("power".into(), PinId(1)),
            ("obj".into(), PinId(2)),
            ("name".into(), PinId(3)),
        ]
    );
    engine
        .set_node_property(
            id,
            "template",
            "{{ power }}{{ text.name }}{{ text }}".into(),
        )
        .unwrap();
    assert_eq!(
        pins(&engine, id),
        vec![("power".into(), PinId(1)), ("name".into(), PinId(3))]
    );
    inline(&mut engine, id, "power", 42.into());
    inline(&mut engine, id, "name", "sensor".into());
    engine
        .set_node_property(id, "template", "{{ text.power }}|{{ name | text }}".into())
        .unwrap();
    assert_eq!(render(&mut engine, id), Some("42|sensor".into()));

    let (mut engine, id) = setup("{% set text = 'local' %}{{ text }}|{{ x }}");
    assert_eq!(pins(&engine, id), vec![("x".into(), PinId(1))]);
    inline(&mut engine, id, "x", 1.into());
    assert_eq!(render(&mut engine, id), Some("local|1".into()));
}

#[test]
fn a_new_node_has_a_value_input_while_stored_templates_are_restored_as_written() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "text/template", &[]).unwrap();
    assert_eq!(pins(&engine, id).into_iter().map(|(n, _)| n).collect::<Vec<_>>(), ["value"]);
    assert!(engine.node_definition(id).unwrap().adaptive);
    let empty = NodeId::generate();
    engine.create_node(empty, "text/template", &[("template".into(), "".into())]).unwrap();
    assert!(pins(&engine, empty).is_empty());
    let stored = NodeId::generate();
    engine.create_node(stored, "text/template", &[("template".into(), "{{ a }}".into())]).unwrap();
    assert_eq!(pins(&engine, stored).into_iter().map(|(n, _)| n).collect::<Vec<_>>(), ["a"]);
}
