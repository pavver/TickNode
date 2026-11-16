use super::*;
use crate::{i18n::Locale, state::Theme};

fn instance(
    id: NodeId,
    ty: &str,
    pin: PinId,
    direction: PinDirection,
    properties: Vec<(String, DataValue)>,
) -> NodeInstance {
    NodeInstance {
        model: NodeModel {
            id,
            type_name: ty.into(),
            properties,
            position: Position::default(),
            pins: None,
            size: None,
        },
        definition: NodeDefinition {
            type_name: ty.into(),
            pins: vec![PinDefinition {
                id: pin,
                name: "value".into(),
                direction,
                pin_type: PinType::Data(DataType::Any),
                required: false,
            }],
            ..Default::default()
        },
        unavailable: None,
        frozen: vec![],
    }
}

#[test]
fn bound_paths_formula_outputs_and_trace_types_share_live_field_contracts() {
    let owner = Owner::new();
    owner.with(|| {
        let ed = Editor::new(Theme::Dark, RwSignal::new(Locale::En), true);
        let power = DataType::Named("quantity.power".into());
        let object = DataValue::Object {
            type_name: None,
            fields: [(
                "power".into(),
                ObjectField {
                    value: 1200i64.into(),
                    data_type: Some(power.clone()),
                },
            )]
            .into(),
        };
        let variable = Variable {
            key: VariableKey::global("tree"),
            data_type: DataType::Any,
            value: object.clone(),
        };
        ed.variables.update(|vars| {
            vars.insert(variable.key.clone(), variable);
        });
        ed.plugin_values.set(vec![PluginValue {
            path: "ha.sensor.one".into(),
            value: Some(object),
        }]);
        ed.value_entry_fields.update(|cache| {
            cache.insert(
                ("ha".into(), "sensor.one".into()),
                vec![ValueField {
                    path: "power".into(),
                    title: "Power".into(),
                    data_type: DataType::Any,
                }],
            );
        });
        let source = NodeId::generate();
        let sink = NodeId::generate();
        ed.nodes.update(|nodes| {
            nodes.insert(
                source,
                ClientNode::new(instance(
                    source,
                    "math/expression",
                    PinId(0),
                    PinDirection::Output,
                    vec![(
                        "expression".into(),
                        "var.tree.power + ha.sensor.one.power".into(),
                    )],
                )),
            );
            nodes.insert(
                sink,
                ClientNode::new(instance(
                    sink,
                    "text/template",
                    PinId(1),
                    PinDirection::Input,
                    vec![("var:1".into(), "var.tree.power".into())],
                )),
            );
        });
        assert_eq!(
            ed.bound_input(sink, PinId(1)),
            Some((1200i64.into(), power.clone()))
        );
        assert_eq!(ed.source_type(sink, PinId(1)), Some(power.clone()));
        ed.node_tracked(sink)
            .unwrap()
            .instance
            .update(|i| i.model.properties = vec![("var:1".into(), "ha.sensor.one.power".into())]);
        assert_eq!(
            ed.bound_input(sink, PinId(1)),
            Some((1200i64.into(), power.clone()))
        );
        assert_eq!(ed.source_type(sink, PinId(1)), Some(power.clone()));
        ed.data.set(vec![ConnectionModel {
            src_node: source,
            src_pin: PinId(0),
            dst_node: sink,
            dst_pin: PinId(1),
        }]);
        assert_eq!(ed.source_type(sink, PinId(1)), Some(power.clone()));
        assert_eq!(
            crate::node_status::trace_type(ed, source, PinId(0), PinDirection::Output),
            power
        );
    });
}
