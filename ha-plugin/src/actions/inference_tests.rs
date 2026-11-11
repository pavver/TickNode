use super::*;
use crate::entities::test_state;

fn select(action: &str, target: &str, attributes: Value, selector: Value) -> ActionNode {
    let (domain, service) = action.split_once('.').unwrap();
    let field = if domain == "climate" {
        "temperature"
    } else {
        "value"
    };
    crate::with_hub(|hub, _| {
        hub.services = services(&json!({domain:{service:{"target":{},"fields":{
            field:{"required":true,"selector":selector}
        }}}}));
        hub.live
            .snapshot(&[test_state(&format!("{domain}.test"), "1.5", attributes)]);
    });
    let mut node = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    node.set_property("target", target.into());
    node.set_property("action", action.into());
    node
}

#[test]
fn unitless_number_selectors_snapshot_the_single_targets_quantity() {
    for domain in ["number", "input_number"] {
        let node = select(
            &format!("{domain}.set_value"),
            &format!("{domain}.test"),
            json!({"device_class":"power","unit_of_measurement":"kW"}),
            json!({"number":{}}),
        );
        let field = &node.service.fields[0];
        assert_eq!(field.data_type, DataType::Named("quantity.power".into()));
        assert_eq!(field.unit.as_deref(), Some("kW"));
        assert_eq!(field.json(&1500.0.into()), Some(json!(1.5)));
        assert_eq!(node.definition().pins[2].id, PinId(2));
        assert!(node.definition().pins[2].required);

        let properties = node.properties();
        crate::with_hub(|hub, _| {
            hub.services.clear();
            hub.live.reset();
        });
        let mut restored = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
        for (name, value) in properties.clone() {
            restored.set_property(&name, value);
        }
        assert_eq!(restored.properties(), properties);
        assert_eq!(restored.definition(), node.definition());
        assert_eq!(
            restored.service.fields[0].json(&1500.0.into()),
            Some(json!(1.5))
        );
    }
}

#[test]
fn climate_setpoint_uses_the_entity_temperature_unit_and_converts_from_canonical() {
    for attributes in [
        json!({"temperature_unit":"°F"}),
        json!({"unit_of_measurement":"°F"}),
    ] {
        let node = select(
            "climate.set_temperature",
            "climate.test",
            attributes,
            json!({"number":{}}),
        );
        assert_eq!(
            node.service.fields[0].data_type,
            DataType::Named("quantity.temperature".into())
        );
        assert_eq!(node.service.fields[0].unit.as_deref(), Some("°F"));
        let result = node.service.fields[0]
            .json(&20.0.into())
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((result - 68.0).abs() < 1e-10);
    }
}

#[test]
fn target_inference_does_not_replace_selector_units_or_infer_ambiguous_targets() {
    for (target, attributes, selector) in [
        (
            "number.test,number.other",
            json!({"unit_of_measurement":"kW"}),
            json!({"number":{}}),
        ),
        (
            "number.missing",
            json!({"unit_of_measurement":"kW"}),
            json!({"number":{}}),
        ),
        (
            "number.test",
            json!({"unit_of_measurement":"widgets"}),
            json!({"number":{}}),
        ),
        ("number.test", json!({}), json!({"number":{}})),
        (
            "number.test",
            json!({"unit_of_measurement":"kW"}),
            json!({"number":{"unit_of_measurement":"widgets"}}),
        ),
    ] {
        let node = select("number.set_value", target, attributes, selector);
        assert_eq!(node.service.fields[0].data_type, DataType::Number);
        assert!(node.service.fields[0].unit.is_none());
    }
    let node = select(
        "number.set_value",
        "number.test",
        json!({"unit_of_measurement":"kW"}),
        json!({"number":{"unit_of_measurement":"W"}}),
    );
    assert_eq!(node.service.fields[0].unit.as_deref(), Some("W"));
    assert_eq!(
        node.service.fields[0]
            .json(&1500.0.into())
            .unwrap()
            .as_f64(),
        Some(1500.0)
    );
    let node = select(
        "number.set_value",
        "number.test",
        json!({"unit_of_measurement":"kW"}),
        json!({"text":{}}),
    );
    assert_eq!(node.service.fields[0].data_type, DataType::String);
    let node = select(
        "number.increment",
        "number.test",
        json!({"unit_of_measurement":"kW"}),
        json!({"number":{}}),
    );
    assert_eq!(node.service.fields[0].data_type, DataType::Number);
}

#[test]
fn legacy_snapshots_override_live_inference_even_when_the_catalog_is_available() {
    let mut node = select(
        "number.set_value",
        "number.test",
        json!({"unit_of_measurement":"kW"}),
        json!({"number":{}}),
    );
    node.set_property("fields", super::node::strings(["value:number".into()]));
    assert_eq!(node.service.fields[0].data_type, DataType::Number);
    assert!(node.service.fields[0].unit.is_none());
    assert_eq!(
        node.service.fields[0]
            .json(&1500.0.into())
            .unwrap()
            .as_f64(),
        Some(1500.0)
    );
    // Live updates and target edits do not silently change a saved pin contract.
    node.set_property("target", "number.other".into());
    assert_eq!(node.service.fields[0].data_type, DataType::Number);
}

#[test]
fn synthetic_state_is_a_required_positional_field_with_an_offline_snapshot() {
    for effect in super::effects::STATE_ACTIONS {
        crate::with_hub(|hub, _| hub.services.clear());
        let mut node = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
        node.set_property("action", format!("{}.set_state", effect.domain).into());
        let pin = &node.definition().pins[2];
        assert_eq!(pin.id, PinId(2));
        assert_eq!(pin.name, "State");
        assert_eq!(pin.pin_type, ticknode_shared::PinType::Data(DataType::Bool));
        assert!(pin.required);
        let mut restored = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
        for (name, value) in node.properties() {
            restored.set_property(&name, value);
        }
        assert_eq!(restored.definition(), node.definition());
        assert_eq!(restored.properties(), node.properties());
        let key = format!(
            "node-ha-action-prop-action-option-{}-set-state =",
            ticknode_shared::fluent_slug(effect.domain)
        );
        assert!(include_str!("../../locales/uk.ftl").contains(&key));
    }
}
