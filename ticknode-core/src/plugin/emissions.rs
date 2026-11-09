use std::sync::{Arc, Mutex};
use ticknode_shared::*;
use crate::TickEngine;
use super::{lock, wit};
use super::runtime::Runtime;
use super::outputs::validate_emission_outputs;

impl TickEngine {
    /// Calls `plugin-tick` of every enabled plugin and applies the returned emissions. Runs once per tick,
    /// after finished tasks are delivered and before event sources are polled.
    pub(crate) fn tick_plugins(&mut self, next_tick_queue: &mut std::collections::VecDeque<(NodeId, PinId)>) {
        let active: Vec<(String, Arc<Mutex<Runtime>>)> = self.plugins.iter()
            .filter(|(_, p)| p.active)
            .map(|(name, p)| (name.clone(), p.runtime.clone()))
            .collect();
        for (name, runtime) in active {
            let result = lock(&runtime).plugin_tick(&mut self.host_state);
            match result {
                Ok(emissions) => self.apply_emissions(&name, emissions, next_tick_queue),
                Err(reason) => self.host_state.log(NodeId(uuid::Uuid::nil()), format!("[ERROR] Plugin {name} crashed in plugin-tick: {reason}")),
            }
        }
    }

    /// Applies emissions as if the target nodes had executed. An emission for a node that does not exist,
    /// belongs to another plugin (or is a stub) is dropped, as is every invalid pin or value; each drop logs an error.
    pub(super) fn apply_emissions(&mut self, plugin: &str, emissions: Vec<wit::Emission>, next_tick_queue: &mut std::collections::VecDeque<(NodeId, PinId)>) {
        for emission in emissions {
            let id = NodeId(uuid::Uuid::from_u64_pair(emission.node.0, emission.node.1));
            let definition = self.nodes.get(&id).map(|n| n.definition());
            let Some(definition) = definition.filter(|d| self.registry.owner_of(&d.type_name) == Some(plugin)) else {
                self.host_state.log(id, format!("[ERROR] Plugin {plugin} emitted outputs for node {id:?}, which is not one of its nodes: dropped"));
                continue;
            };
            if self.skip_frozen(id) { continue; }
            let (outputs, errors) = validate_emission_outputs(&definition, emission.outputs);
            for e in errors {
                self.host_state.log(id, format!("[ERROR] {e}"));
            }
            if !outputs.is_empty() {
                self.last_tick.charge(definition.cost);
                self.apply_outputs(id, Vec::new(), outputs, next_tick_queue);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::convert::encode_value;

    #[test]
    fn frozen_emissions_are_dropped_until_confirmation() {
        let mut engine = TickEngine::new();
        engine.registry.register_owned("probe", Box::new(|id| Box::new(crate::builtin::ConstNode { id, value: 1i64.into() })));
        let id = NodeId::generate();
        engine.create_node(id, "const", &[]).unwrap();
        let mut graph = engine.export_graph();
        graph.nodes[0].pins.as_mut().unwrap()[0].kind = PinKind::Data(DataType::Object);
        engine.load_graph(&graph);
        assert!(!engine.frozen_changes(id).is_empty());
        let mut queue = Default::default();
        let emission = || wit::Emission {
            node: id.0.as_u64_pair(), outputs: vec![wit::Output::Data((0, encode_value(&42i64.into())))],
        };
        engine.apply_emissions("probe", vec![emission(), emission()], &mut queue);
        assert!(!engine.data_cache.contains_key(&id));
        assert!(engine.executed.is_empty());
        assert_eq!(engine.host_state.logs.iter().filter(|(_, s)| s.contains("Node is frozen:")).count(), 1);
        engine.confirm_pins(id).unwrap();
        engine.apply_emissions("probe", vec![emission()], &mut queue);
        assert_eq!(engine.data_cache[&id][&PinId(0)], 42i64.into());
        assert_eq!(engine.executed.len(), 1);
    }
}
