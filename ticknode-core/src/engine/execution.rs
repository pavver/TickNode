mod pull;
mod outputs;

use std::collections::{HashMap, VecDeque};
use ticknode_shared::*;
use crate::{NodeContext, TickEngine};

impl TickEngine {
    pub fn trigger_node(&mut self, node_id: NodeId, pin_id: PinId) {
        self.execution_queue.push_back((node_id, pin_id));
    }

    // --- Execution ---

    /// A node with an unset required input is not executed. An input is considered set if a source is
    /// connected to it or an inline value is set on the node. A connected source with no value yet skips
    /// the node silently; a missing input logs an error (once, until the state changes).
    fn skip_for_required(&mut self, node_id: NodeId, def: &NodeDefinition, inputs: &HashMap<PinId, Option<DataValue>>) -> bool {
        let mut pending = false;
        for pin in def.pins.iter().filter(|p| p.required && p.direction == PinDirection::Input && p.pin_type != PinType::Exec) {
            match inputs.get(&pin.id) {
                Some(Some(_)) => {}
                Some(None) => pending = true,
                None => {
                    if self.required_reported.insert(node_id) {
                        self.host_state.log(node_id, format!("[ERROR] Required input \"{}\" is not set", pin.name));
                    }
                    return true;
                }
            }
        }
        if !pending {
            self.required_reported.remove(&node_id);
        }
        pending
    }

    pub fn tick(&mut self) {
        self.last_tick = Default::default();
        self.pure_cache.clear();
        self.executed.clear();
        self.executed_index.clear();
        self.host_state.current_tick += 1;
        let mut next_tick_queue = VecDeque::new();

        self.deliver_finished(&mut next_tick_queue);

        // Plugin-level events (connections) and emissions to the plugins' nodes.
        for node in self.nodes.values_mut() {
            node.ensure_live();
        }
        self.tick_plugins(&mut next_tick_queue);

        self.poll_sources();

        self.run_queue(&mut next_tick_queue);
    }

    pub(crate) fn run_queue(&mut self, next_tick_queue: &mut VecDeque<(NodeId, PinId)>) {
        let mut current_tick_queue = std::mem::take(&mut self.execution_queue);
        while let Some((node_id, trigger_pin)) = current_tick_queue.pop_front() {
            let Some(def) = self.definition_arc(node_id) else { continue };
            if self.budget.should_defer(self.last_tick.spent, def.cost) {
                current_tick_queue.push_front((node_id, trigger_pin));
                self.last_tick.deferred = current_tick_queue.len();
                self.execution_queue.append(&mut current_tick_queue);
                break;
            }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            if self.skip_for_required(node_id, &def, &inputs) {
                continue;
            }
            let traced = trace_inputs(&inputs);
            let inputs = self.native_inputs(node_id, inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.execute(Some(trigger_pin), &mut ctx);
            self.last_tick.charge(def.cost);
            self.apply_outputs(node_id, traced, outputs, next_tick_queue);
        }
        self.execution_queue.append(next_tick_queue);
    }

    fn deliver_finished(&mut self, next_tick_queue: &mut VecDeque<(NodeId, PinId)>) {
        // Completed tasks (delays, HTTP) are delivered to their owning nodes.
        for (node_id, task, result) in self.host_state.take_finished() {
            let Some(def) = self.definition_arc(node_id) else { continue };
            self.last_tick.charge(def.cost);
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            let traced = trace_inputs(&inputs);
            let inputs = self.native_inputs(node_id, inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.on_task(task, result, &mut ctx);
            self.apply_outputs(node_id, traced, outputs, next_tick_queue);
        }

    }

    fn poll_sources(&mut self) {
        let mut fired = Vec::new();
        let sources: Vec<_> = self.nodes.iter().filter(|(_, node)| node.polls()).map(|(id, _)| *id).collect();
        for id in sources {
            let Some(def) = self.definition_arc(id) else { continue };
            self.last_tick.charge(def.cost);
            let node = self.nodes.get_mut(&id).expect("event source exists");
            if let Some(pin) = node.poll(&mut self.host_state) {
                fired.push((id, pin));
            }
        }
        for (id, pin) in fired {
            self.trigger_node(id, pin);
        }

    }
}

fn trace_inputs(inputs: &HashMap<PinId, Option<DataValue>>) -> Vec<(PinId, Option<DataValue>)> {
    let mut traced: Vec<_> = inputs.iter().map(|(p, v)| (*p, v.clone())).collect();
    traced.sort_by_key(|(p, _)| p.0);
    traced
}
