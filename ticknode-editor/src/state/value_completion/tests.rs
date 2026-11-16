use super::*;
use crate::{i18n, inline::variable::binding_presentation, ui::ControlState};

fn catalog(namespace: &str) -> ValueCatalog {
    ValueCatalog {
        namespace: namespace.into(),
        entries: ["kitchen", "bedroom"]
            .map(|name| ValueEntrySummary {
                key: format!("sensor.{name}"),
                title: format!("{name} temperature"),
                icon: Some("mdi:thermometer".into()),
            })
            .into(),
    }
}

fn fields() -> Vec<ValueField> {
    vec![ValueField {
        path: "state".into(),
        title: "State".into(),
        data_type: DataType::Named("quantity.temperature".into()),
    }]
}

fn editor() -> Editor {
    let ed = Editor::new(Theme::Dark, RwSignal::new(Locale::En), true);
    let types = TypeRegistry::from_definitions(quantity::catalog().iter().cloned());
    i18n::set_known_types(&types);
    ed.types.set(types);
    ed.apply(ServerMessage::ValueCatalogs(vec![
        catalog("ha"),
        catalog("other"),
    ]));
    ed
}

#[test]
fn requests_are_entry_scoped_deduplicated_cached_and_invalidated_per_namespace() {
    let owner = Owner::new();
    owner.with(|| {
        let ed = editor();
        ed.request_value_fields("{{ ha.sensor. }}", true);
        assert!(ed.value_fields_pending.get_value().is_empty());
        ed.request_value_fields(
            "{{ ha.sensor.kitchen.state }} {{ text.ha.sensor.kitchen.state }}",
            true,
        );
        ed.request_value_fields("ha.sensor.kitchen.state", false);
        assert_eq!(
            ed.value_fields_pending.get_value(),
            [("ha".into(), "sensor.kitchen".into())].into()
        );
        assert!(ed.value_entry_fields.get().is_empty());
        assert!(ed.with_value_tree(|ctx| ctx.fields("ha", "sensor.bedroom").is_none()));
        ed.apply(ServerMessage::ValueEntryFields {
            namespace: "ha".into(),
            key: "sensor.kitchen".into(),
            fields: fields(),
        });
        assert!(ed.value_fields_pending.get_value().is_empty());
        ed.request_value_fields("ha.sensor.kitchen.state", false);
        assert!(ed.value_fields_pending.get_value().is_empty());
        ed.apply(ServerMessage::ValueCatalogs(vec![catalog("other")]));
        assert_eq!(ed.value_entry_fields.get().len(), 1);
        ed.apply(ServerMessage::ValueCatalogs(vec![catalog("ha")]));
        assert!(ed.value_entry_fields.get().is_empty());
        ed.request_value_fields("{{ text.ha.sensor.kitchen.state }}", true);
        assert_eq!(ed.value_fields_pending.get_value().len(), 1);
    });
}

#[test]
fn chips_show_entity_title_declared_type_and_compatibility_without_telemetry() {
    let owner = Owner::new();
    owner.with(|| {
        let ed = editor();
        ed.apply(ServerMessage::ValueEntryFields {
            namespace: "ha".into(),
            key: "sensor.kitchen".into(),
            fields: fields(),
        });
        for locale in [Locale::En, Locale::Uk] {
            i18n::with_locale(locale, || {
                let ty = DataType::Named("quantity.temperature".into());
                let presentation = binding_presentation(ed, "ha.sensor.kitchen.state", &ty, false);
                assert!(presentation.label.contains("kitchen temperature"));
                assert!(presentation.label.contains(&ed.type_title(&ty)));
                assert_eq!(presentation.icon, "mdi-thermometer");
                assert_eq!(presentation.state, ControlState::Idle);
                assert!(
                    presentation
                        .tooltip
                        .contains(&i18n::t!("inline-binding-unavailable"))
                );
                let incompatible = binding_presentation(
                    ed,
                    "ha.sensor.kitchen.state",
                    &DataType::Named("quantity.power".into()),
                    false,
                );
                assert_eq!(incompatible.state, ControlState::Error);
                assert_eq!(
                    incompatible.tooltip,
                    i18n::t!(
                        "inline-binding-type-error",
                        source = ed.type_title(&ty),
                        target = ed.type_title(&DataType::Named("quantity.power".into()))
                    )
                );
            });
        }
    });
}

#[test]
fn cached_declarations_drive_preview_and_bound_trace_types_from_sheet_telemetry() {
    let owner = Owner::new();
    owner.with(|| {
        i18n::with_locale(Locale::En, || {
            let ed = editor();
            provide_context(ed);
            ed.apply(ServerMessage::ValueEntryFields {
                namespace: "ha".into(),
                key: "sensor.kitchen".into(),
                fields: fields(),
            });
            let object = DataValue::Object {
                type_name: None,
                fields: [(
                    "state".into(),
                    ObjectField {
                        value: 23i64.into(),
                        data_type: None,
                    },
                )]
                .into(),
            };
            ed.apply(ServerMessage::PluginValues(vec![PluginValue {
                path: "ha.sensor.kitchen".into(),
                value: Some(object.clone()),
            }]));
            let ty = DataType::Named("quantity.temperature".into());
            let (value, actual_ty) = ed.plugin_value("ha.sensor.kitchen.state").unwrap();
            assert_eq!(actual_ty, ty);
            let expected = i18n::value_as_text(&value, &ty);
            let preview = crate::inline::template_field::preview::evaluate_preview(
                "{{ text.ha.sensor.kitchen.state }}",
                &[],
                template::TemplateOptions::default(),
            );
            assert_eq!(
                preview,
                crate::inline::template_field::preview::PreviewStatus::Text(expected.clone())
            );
            let node_id = NodeId::generate();
            ed.nodes.update(|nodes| {
                nodes.insert(
                    node_id,
                    ClientNode::new(NodeInstance {
                        model: NodeModel {
                            id: node_id,
                            type_name: "text/template".into(),
                            properties: vec![("var:1".into(), "ha.sensor.kitchen.state".into())],
                            position: Position::default(),
                            pins: None,
                            size: None,
                        },
                        definition: NodeDefinition {
                            type_name: "text/template".into(),
                            pins: vec![PinDefinition {
                                id: PinId(1),
                                name: "value".into(),
                                direction: PinDirection::Input,
                                pin_type: PinType::Data(DataType::Any),
                                required: false,
                            }],
                            ..Default::default()
                        },
                        unavailable: None,
                        frozen: vec![],
                    }),
                );
            });
            assert_eq!(ed.source_type(node_id, PinId(1)), Some(ty.clone()));
            assert_eq!(
                crate::node_status::trace_type(ed, node_id, PinId(1), PinDirection::Input),
                ty
            );
            ed.apply(ServerMessage::Executed(vec![NodeTrace {
                node_id,
                tick: 1,
                inputs: vec![(PinId(1), Some(value.clone()))],
                outputs: vec![],
                fired: vec![],
            }]));
            let trace_text =
                crate::node_status::pin_status(ed, node_id, PinId(1), PinDirection::Input, false);
            assert!(trace_text.contains(&expected));
            let inputs = crate::inline::template_field::preview::node_preview_inputs(ed, node_id);
            assert_eq!(inputs[0].1, Some(value));
            assert_eq!(
                crate::inline::template_field::preview::evaluate_preview(
                    "{{ text.value }}",
                    &inputs,
                    template::TemplateOptions::default()
                ),
                crate::inline::template_field::preview::PreviewStatus::Text(expected)
            );
            ed.apply(ServerMessage::PluginValues(vec![PluginValue {
                path: "ha.sensor.kitchen".into(),
                value: None,
            }]));
            assert_eq!(ed.source_type(node_id, PinId(1)), Some(ty));
            assert!(ed.bound_input(node_id, PinId(1)).is_none());
        })
    });
}

#[test]
fn namespace_field_completion_localizes_ticknode_type_names_in_both_locales() {
    let owner = Owner::new();
    owner.with(|| {
        i18n::set_runtime_translations(&[(
            "uk".into(),
            include_str!("../../../../ticknode-core/locales/uk.ftl").into(),
        )]);
        let ed = editor();
        ed.apply(ServerMessage::ValueEntryFields {
            namespace: "ha".into(),
            key: "sensor.kitchen".into(),
            fields: fields(),
        });
        for (locale, title) in [(Locale::En, "Temperature"), (Locale::Uk, "Температура")]
        {
            i18n::with_locale(locale, || {
                ed.with_value_tree(|ctx| {
                    ed.types.with(|types| {
                        let source = "{{ text.ha.sensor.kitchen.st";
                        let input = template::CompletionInput {
                            source,
                            caret: source.len(),
                            inputs: &[],
                            variables: &[],
                            types,
                            now: None,
                            labels: Some(&i18n::UiQuantityLabels),
                        };
                        let completion =
                            template::complete::complete_with_catalog(&input, &[], Some(ctx))
                                .unwrap();
                        assert_eq!(completion.items.len(), 1);
                        assert!(
                            completion.items[0].detail.contains(title),
                            "{:?}",
                            completion.items
                        );
                        assert!(completion.items[0].detail.contains("°C"));
                    })
                })
            });
        }
        i18n::set_runtime_translations(&[]);
    });
}
