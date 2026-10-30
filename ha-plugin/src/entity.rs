//! An entity reference whose data outputs are updated by subscriptions, without execution signals.

use ha_core_client::State;
use std::time::Instant;
use ticknode_plugin_sdk::*;

use crate::convert::{STATE_TYPE, missing_state, state_value};
use crate::{CATEGORY, text_property, try_with_hub, with_hub};

pub fn outputs(entity: &str, state: Option<&State>) -> Vec<Output> {
    let value = state.map_or_else(|| missing_state(entity), state_value);
    let text = state.map_or("", |s| s.state.as_str());
    let attrs = field(&value, "attributes")
        .expect("ha.state has attributes")
        .clone();
    let unit = field_string(&attrs, "unit_of_measurement").unwrap_or("");
    let mut outputs = vec![
        Output::Data(PinId(0), text.into()),
        Output::Data(PinId(3), unit.into()),
        Output::Data(PinId(4), attrs),
        Output::Data(PinId(5), value),
    ];
    if let Ok(number) = text.parse::<f64>()
        && number.is_finite()
    {
        outputs.push(Output::Data(PinId(1), number.into()));
    }
    let on = match text {
        "on" | "open" | "home" | "true" | "playing" | "locked" | "detected" | "active"
        | "occupied" | "connected" => Some(true),
        "off" | "closed" | "not_home" | "false" | "paused" | "unlocked" | "clear" | "inactive"
        | "unoccupied" | "disconnected" => Some(false),
        _ => None,
    };
    if let Some(on) = on {
        outputs.push(Output::Data(PinId(2), on.into()));
    }
    outputs
}

pub struct Entity {
    node: NodeId,
    serial: Option<u64>,
    entity: String,
}

impl Entity {
    pub fn new(node: NodeId) -> Self {
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_entity(node)));
        Self {
            node,
            serial,
            entity: String::new(),
        }
    }
}

impl Drop for Entity {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_watch(self.node, serial, Instant::now(), env));
        }
    }
}

impl PluginNode for Entity {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "ha/entity".into(), title: "Entity".into(), category: CATEGORY.into(),
            description: "Keeps the latest state of a Home Assistant entity available as data".into(),
            help: "Choose an entity. Its current state is read on subscription and reconnection; changes update the data outputs without firing an execution signal. Downstream actions can pull the latest values at any time. Number is emitted only for finite numeric states; On only for on/off, open/closed, home/not_home, true/false, playing/paused, locked/unlocked, detected/clear, active/inactive, occupied/unoccupied and connected/disconnected. Other states leave those outputs unchanged (an output cannot be cleared). Unit comes from unit_of_measurement. A removed entity emits an empty State and Entity state. While disconnected the last values remain available; a missing entity is retried every 30 seconds.".into(),
            properties: vec![PropertyDefinition::dynamic("entity_id", "Entity", "entities").described("The Home Assistant entity whose latest state to expose")],
            pins: vec![
                data_out(0, "State", DataType::String), data_out(1, "Number", DataType::Number),
                data_out(2, "On", DataType::Bool), data_out(3, "Unit", DataType::String),
                data_out(4, "Attributes", DataType::Object), data_out(5, "Entity state", DataType::Named(STATE_TYPE.into())),
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("entity_id".into(), self.entity.clone().into())]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "entity_id"
            && let Some(entity) = text_property(value)
        {
            if let Some(serial) = self.serial {
                with_hub(|hub, env| {
                    hub.set_entity(self.node, serial, &entity, Instant::now(), env)
                });
            }
            self.entity = entity;
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::test_state;
    use serde_json::json;

    fn data(outputs: &[Output], pin: u32) -> Option<&DataValue> {
        outputs.iter().find_map(|o| match o {
            Output::Data(p, v) if p.0 == pin => Some(v),
            _ => None,
        })
    }

    #[test]
    fn numeric_boolean_and_text_states_have_only_applicable_outputs() {
        let state = test_state(
            "sensor.temp",
            "21.0",
            json!({"unit_of_measurement":"°C", "custom":42}),
        );
        let out = outputs(&state.entity_id, Some(&state));
        assert_eq!(data(&out, 1), Some(&DataValue::from(21.0)));
        assert_eq!(data(&out, 2), None);
        assert_eq!(data(&out, 3), Some(&DataValue::from("°C")));
        assert_eq!(field_number(data(&out, 4).unwrap(), "custom"), Some(42.0));
        assert_eq!(
            data(&out, 5).unwrap().get_type(),
            DataType::Named(STATE_TYPE.into())
        );
        for (state, on) in [
            ("on", true),
            ("off", false),
            ("open", true),
            ("closed", false),
            ("home", true),
            ("not_home", false),
            ("true", true),
            ("false", false),
            ("playing", true),
            ("paused", false),
            ("locked", true),
            ("unlocked", false),
            ("detected", true),
            ("clear", false),
        ] {
            let out = outputs("light.a", Some(&test_state("light.a", state, json!({}))));
            assert_eq!(data(&out, 2), Some(&DataValue::Bool(on)), "{state}");
            assert!(data(&out, 1).is_none());
            assert!(out.iter().all(|o| matches!(o, Output::Data(..))));
        }
        for state in ["hello", "unavailable", "NaN", "inf", "On"] {
            let out = outputs("sensor.x", Some(&test_state("sensor.x", state, json!({}))));
            assert_eq!(data(&out, 0), Some(&DataValue::from(state)));
            assert!(data(&out, 1).is_none() && data(&out, 2).is_none());
        }
        let out = outputs("sensor.gone", None);
        assert_eq!(data(&out, 0), Some(&DataValue::from("")));
    }
}
