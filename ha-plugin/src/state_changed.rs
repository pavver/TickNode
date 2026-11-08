//! Subscription-driven state-change event node.

use crate::convert::STATE_TYPE;
use crate::hub::{CHANGED, NEW, OLD};
use crate::queue::Mode;
use crate::{CATEGORY, entities, text_property, with_hub};
use std::time::Instant;
use ticknode_plugin_sdk::*;

// --- ha/state_changed ---

pub(super) struct StateChanged {
    node: NodeId,
    /// `None` for the nil-id node used to read the definition: it never touches the hub.
    serial: Option<u64>,
    entity: String,
    mode: Mode,
}

impl StateChanged {
    pub(super) fn new(node: NodeId) -> Self {
        let mode = Mode::default();
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_watch(node, mode)));
        Self {
            node,
            serial,
            entity: String::new(),
            mode,
        }
    }
}

impl Drop for StateChanged {
    fn drop(&mut self) {
        crate::runtime::remove_watch(self.node, self.serial);
    }
}

impl PluginNode for StateChanged {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 2,
            properties: vec![
                PropertyDefinition::dynamic("entity_id", "Entity", entities::ENTITIES_SOURCE)
                    .described("The Home Assistant entity to watch. Pick one from the list or type an entity id"),
                PropertyDefinition::select("mode", "Mode", &[("every", "Every change"), ("latest", "Latest only")])
                    .described("What happens when several changes arrive in the same tick"),
            ],
            type_name: "ha/state_changed".into(),
            title: "HA State Changed".into(),
            category: CATEGORY.into(),
            description: "Fires when a Home Assistant entity changes".into(),
            help: "Event source: sends a signal on Changed every time the entity from the entity_id property changes its state or attributes in Home Assistant. \
New is the state after the change and Old the state before it. If the entity did not exist before (or was removed), the corresponding output is cleared.\n\n\
The mode property says what happens when several changes arrive in the same tick: \"every\" (default) sends one signal per change, in order, one per tick; \"latest\" sends only the newest change of the tick.\n\n\
Needs the Home Assistant connection to be set up in the plugin settings. While the connection is down no changes are seen, and after it returns a change that happened meanwhile is delivered as one signal. \
A nonexistent entity is retried every 30 seconds.".into(),
            pins: vec![
                exec_out(CHANGED.0, "Changed"),
                data_out(NEW.0, "New", DataType::Named(STATE_TYPE.into())),
                data_out(OLD.0, "Old", DataType::Named(STATE_TYPE.into())),
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("entity_id".into(), DataValue::String(self.entity.clone())),
            ("mode".into(), DataValue::String(self.mode.as_str().into())),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else {
            return;
        };
        match name {
            "entity_id" => {
                if let Some(serial) = self.serial {
                    with_hub(|hub, env| {
                        hub.set_entity(self.node, serial, &text, Instant::now(), env)
                    });
                }
                self.entity = text;
            }
            "mode" => self.set_mode(&text),
            _ => {}
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
}

impl StateChanged {
    fn set_mode(&mut self, text: &str) {
        match Mode::parse(text) {
            Some(mode) => {
                self.mode = mode;
                if let Some(serial) = self.serial {
                    with_hub(|hub, _| hub.set_mode(self.node, serial, mode));
                }
            }
            None => host::warn(&format!(
                "Unknown mode \"{text}\" for ha/state_changed (expected \"every\" or \"latest\"): keeping \"{}\"",
                self.mode.as_str()
            )),
        }
    }
}
