//! Typed entity event source, fed by the hub's ordered subscription emissions.

mod value;
pub(crate) use value::unit_mismatch;
pub use value::{ValueType, infer_value_type, typed_value};

use ha_core_client::{State, StateChange};
use std::time::Instant;
use ticknode_plugin_sdk::*;

use crate::convert::{STATE_TYPE, state_value};
use crate::{CATEGORY, text_property, with_hub};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FireOn {
    #[default]
    State,
    Any,
}

impl FireOn {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "state" => Some(Self::State),
            "any" => Some(Self::Any),
            _ => None,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Any => "any",
        }
    }
}

fn value_output(pin: u32, state: Option<&State>, value_type: ValueType) -> Output {
    let pin = PinId(pin);
    state
        .and_then(|s| typed_value(s, value_type))
        .map_or(Output::Clear(pin), |value| Output::Data(pin, value))
}

/// Baselines update current data, preserving Previous and never firing Changed.
pub fn outputs(state: Option<&State>, value_type: ValueType) -> Vec<Output> {
    let Some(state) = state else {
        return [1, 3, 4, 5]
            .into_iter()
            .map(|pin| Output::Clear(PinId(pin)))
            .collect();
    };
    let value = state_value(state);
    let attrs = field(&value, "attributes")
        .expect("ha.state has attributes")
        .clone();
    let unit = field_string(&attrs, "unit_of_measurement").unwrap_or("");
    vec![
        value_output(1, Some(state), value_type),
        Output::Data(PinId(3), unit.into()),
        Output::Data(PinId(4), attrs),
        Output::Data(PinId(5), value),
    ]
}

pub fn change_outputs(change: &StateChange, value_type: ValueType, fire_on: FireOn) -> Vec<Output> {
    let mut out = outputs(change.new_state.as_ref(), value_type);
    out.push(value_output(2, change.old_state.as_ref(), value_type));
    if fire_on == FireOn::Any
        || match (&change.old_state, &change.new_state) {
            (Some(old), Some(new)) => old.state != new.state,
            _ => true,
        }
    {
        out.push(Output::Exec(PinId(0)));
    }
    out
}

pub struct Entity {
    node: NodeId,
    serial: Option<u64>,
    entity: String,
    value_type: ValueType,
    fire_on: FireOn,
}

impl Entity {
    pub fn new(node: NodeId) -> Self {
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_entity(node)));
        Self {
            node,
            serial,
            entity: String::new(),
            value_type: ValueType::Text,
            fire_on: FireOn::State,
        }
    }
}

impl Drop for Entity {
    fn drop(&mut self) {
        crate::runtime::remove_watch(self.node, self.serial);
    }
}

impl PluginNode for Entity {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 2,
            type_name: "ha/entity".into(), title: "Entity".into(), category: CATEGORY.into(),
            description: "Fires when a Home Assistant entity changes and exposes its typed value".into(),
            help: "Choose an entity. Changed fires on state text changes by default; Any update also fires on attribute-only updates. Each event updates Value and Previous (the value before the change), in order, at most one per tick. Value type is auto-detected when choosing a known entity; you can override it with a physical quantity, Number, On/Off, Text, Date and time (RFC 3339) or Duration. Duration sensor units convert to elapsed time. Quantity values use canonical units (for example kW becomes W); each state uses its own unit. A missing or incompatible unit clears the affected pin and logs a warning until recovery. Unknown, unavailable or non-convertible values clear the corresponding pin. Unit comes from unit_of_measurement. Attributes and Entity state expose the full current state. Subscription and reconnect snapshots update current data without firing Changed or changing Previous. A removed entity clears current data. While offline the last values remain available; missed events are not replayed. A missing entity subscription is retried every 30 seconds.".into(),
            properties: vec![
                PropertyDefinition::dynamic("entity_id", "Entity", "entities")
                    .described("The Home Assistant entity to watch"),
                value::type_property(),
                PropertyDefinition::select("fire_on", "Fire on", &[("state", "State changes"), ("any", "Any update")])
                    .described("When Changed fires; data outputs update on every event"),
            ],
            pins: vec![
                exec_out(0, "Changed"), data_out(1, "Value", self.value_type.data_type()),
                data_out(2, "Previous", self.value_type.data_type()), data_out(3, "Unit", DataType::String),
                data_out(4, "Attributes", DataType::Object), data_out(5, "Entity state", DataType::Named(STATE_TYPE.into())),
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("entity_id".into(), self.entity.clone().into()),
            ("value_type".into(), self.value_type.as_str().into()),
            ("fire_on".into(), self.fire_on.as_str().into()),
        ]
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        crate::validation::schema(&self.definition(), name, value)?;
        if name == "entity_id"
            && let DataValue::String(text) = value
            && !text.is_empty()
            && !crate::validation::entity_id(text)
        {
            return Err(crate::validation::error(
                "entity-id",
                "Enter an entity ID as domain.object_id",
            ));
        }
        Ok(())
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else {
            return;
        };
        match name {
            "entity_id" => {
                self.entity = text;
                if let Some(inferred) =
                    with_hub(|hub, _| hub.live.get(&self.entity).map(infer_value_type))
                {
                    self.value_type = inferred;
                }
            }
            "value_type" => {
                let Some(value_type) = ValueType::parse(&text) else {
                    return;
                };
                self.value_type = value_type;
            }
            "fire_on" => {
                let Some(fire_on) = FireOn::parse(&text) else {
                    return;
                };
                self.fire_on = fire_on;
            }
            _ => return,
        }
        if let Some(serial) = self.serial {
            with_hub(|hub, env| {
                hub.set_entity_options(self.node, serial, self.value_type, self.fire_on);
                hub.set_entity(self.node, serial, &self.entity, Instant::now(), env);
            });
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod quantity_tests;

#[cfg(test)]
mod duration_tests;
