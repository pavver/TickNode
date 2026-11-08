use std::collections::{HashMap, VecDeque};
use ticknode_shared::*;
use crate::{NodeContext, TickEngine};

impl TickEngine {
    pub fn trigger_node(&mut self, node_id: NodeId, pin_id: PinId) {
        self.execution_queue.push_back((node_id, pin_id));
    }

    // --- Execution ---

    pub(crate) fn pull_data(&mut self, node_id: NodeId, pin_id: PinId, stack: &mut Vec<NodeId>) -> Option<DataValue> {
        if stack.contains(&node_id) { return None; }
        if let Some((parent, field)) = self.split_output_field(node_id, pin_id) {
            let DataValue::Object { fields, .. } = self.pull_data(node_id, parent, stack)? else { return None };
            return fields.get(&field).map(|f| f.value.clone());
        }

        // Check whether this is an input pin connected to another node's output.
        // If so, recursively pull data from the source.
        let incoming = self.data_connections.iter()
            .find(|c| c.dst_node == node_id && c.dst_pin == pin_id)
            .copied();
        if let Some(c) = incoming {
            stack.push(node_id);
            let val = self.pull_data(c.src_node, c.src_pin, stack);
            stack.pop();
            return val;
        }

        if let Some(cached_node_vals) = self.pure_cache.get(&node_id) {
            if let Some(val) = cached_node_vals.get(&pin_id) {
                return Some(val.clone());
            }
        }

        let is_pure = self.nodes.get(&node_id).map(|n| n.is_pure()).unwrap_or(false);
        if is_pure {
            stack.push(node_id);
            let definition = self.node_definition(node_id)?;
            let inputs = self.pull_inputs(node_id, stack);
            if self.skip_for_required(node_id, &definition, &inputs) {
                stack.pop();
                return None;
            }
            let traced = trace_inputs(&inputs);
            let inputs = self.native_inputs(node_id, inputs);
            let mut node = self.nodes.remove(&node_id)?;
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let outputs = node.execute(None, &mut ctx);
            self.nodes.insert(node_id, node);
            stack.pop();

            let mut node_outputs = HashMap::new();
            for out in outputs {
                match out {
                    NodeOutput::Data(p, v) => { node_outputs.insert(p, v); }
                    NodeOutput::Clear(p) => { node_outputs.remove(&p); }
                    _ => {}
                }
            }
            let target_val = node_outputs.get(&pin_id).cloned();
            self.pure_cache.insert(node_id, node_outputs);
            self.record_trace(node_id, traced, Vec::new());
            target_val
        } else {
            self.data_cache.get(&node_id).and_then(|m| m.get(&pin_id)).cloned()
        }
    }

    pub(crate) fn pull_inputs(&mut self, node_id: NodeId, stack: &mut Vec<NodeId>) -> HashMap<PinId, Option<DataValue>> {
        let connections: Vec<_> = self.data_connections.iter().filter(|c| c.dst_node == node_id).copied().collect();
        let mut inputs: HashMap<PinId, Option<DataValue>> = connections.into_iter()
            .map(|c| {
                let mut value = self.pull_data(c.src_node, c.src_pin, stack);
                if let (Ok(PinType::Data(src)), Ok(PinType::Data(dst))) = (
                    self.find_pin(c.src_node, c.src_pin, PinDirection::Output),
                    self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input),
                ) && let Some(conversion) = self.types.convertible(&src, &dst)
                    && let Some(original) = value.take()
                {
                    value = conversion.apply(original);
                    if value.is_none()
                        && let Some(pin) = self.node_definition(node_id).and_then(|d| d.pins.into_iter().find(|p| p.id == c.dst_pin && p.direction == PinDirection::Input && p.required))
                        && self.required_reported.insert(node_id)
                    {
                        self.host_state.log(node_id, format!("[ERROR] Required input \"{}\" could not be converted", pin.name));
                    }
                }
                (c.dst_pin, value)
            })
            .collect();
        for (pin, value) in self.input_values.get(&node_id).into_iter().flatten() {
            inputs.entry(PinId(*pin)).or_insert_with(|| Some(value.clone()));
        }
        self.assemble_split_inputs(node_id, &mut inputs);
        inputs
    }

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
        self.pure_cache.clear();
        self.executed.clear();
        self.host_state.current_tick += 1;
        let mut next_tick_queue = VecDeque::new();

        // Completed tasks (delays, HTTP) are delivered to their owning nodes.
        for (node_id, task, result) in self.host_state.take_finished() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            let traced = trace_inputs(&inputs);
            let inputs = self.native_inputs(node_id, inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.on_task(task, result, &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }

        // Plugin-level events (connections) and emissions to the plugins' nodes.
        for node in self.nodes.values_mut() {
            node.ensure_live();
        }
        self.tick_plugins(&mut next_tick_queue);

        let mut fired = Vec::new();
        for (id, node) in self.nodes.iter_mut() {
            if let Some(pin) = node.poll(&mut self.host_state) {
                fired.push((*id, pin));
            }
        }
        for (id, pin) in fired {
            self.trigger_node(id, pin);
        }

        let mut current_tick_queue = std::mem::take(&mut self.execution_queue);
        while let Some((node_id, trigger_pin)) = current_tick_queue.pop_front() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            if let Some(def) = self.node_definition(node_id) && self.skip_for_required(node_id, &def, &inputs) {
                continue;
            }
            let traced = trace_inputs(&inputs);
            let inputs = self.native_inputs(node_id, inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.execute(Some(trigger_pin), &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }
        self.execution_queue.extend(next_tick_queue);
    }

    /// Exec outputs are queued for the next tick; data and variables are applied immediately.
    pub(crate) fn apply_outputs(
        &mut self,
        node_id: NodeId,
        inputs: Vec<(PinId, Option<DataValue>)>,
        outputs: Vec<NodeOutput>,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) {
        let mut state_changed = false;
        let mut fired = Vec::new();
        for output in outputs {
            match output {
                NodeOutput::Execution(out_exec_pin) => {
                    fired.push(out_exec_pin);
                    for c in &self.exec_connections {
                        if c.src_node == node_id && c.src_pin == out_exec_pin {
                            next_tick_queue.push_back((c.dst_node, c.dst_pin));
                        }
                    }
                }
                NodeOutput::Data(pin_id, value) => {
                    self.data_cache.entry(node_id).or_default().insert(pin_id, value);
                    state_changed = true;
                }
                NodeOutput::Clear(pin_id) => {
                    if let Some(values) = self.data_cache.get_mut(&node_id) {
                        values.remove(&pin_id);
                    }
                    state_changed = true;
                }
                NodeOutput::Variable(name, value) => {
                    self.variables.insert(name, value);
                    state_changed = true;
                }
            }
        }
        // Pure nodes evaluated earlier in this tick might depend on the updated data.
        if state_changed {
            self.pure_cache.clear();
        }
        self.record_trace(node_id, inputs, fired);
    }

    /// If a node executed multiple times in a tick, the last execution's data is kept while fired exec pins accumulate.
    fn record_trace(&mut self, node_id: NodeId, inputs: Vec<(PinId, Option<DataValue>)>, fired: Vec<PinId>) {
        let mut outputs: Vec<(PinId, DataValue)> = self.data_cache.get(&node_id).into_iter().flatten()
            .chain(self.pure_cache.get(&node_id).into_iter().flatten())
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        // Telemetry uses the same visible pins as the editor, without evaluating any node again.
        if let Some(def) = self.node_definition(node_id) {
            let projected: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Output)
                .filter_map(|p| {
                    let (parent, field) = self.split_output_field(node_id, p.id)?;
                    let DataValue::Object { fields, .. } = &outputs.iter().find(|(id, _)| *id == parent)?.1 else { return None };
                    Some((p.id, fields.get(&field)?.value.clone()))
                }).collect();
            outputs.retain(|(id, _)| def.pins.iter().any(|p| p.id == *id && p.direction == PinDirection::Output));
            outputs.extend(projected);
        }
        outputs.sort_by_key(|(p, _)| p.0);
        outputs.dedup_by_key(|(p, _)| *p);
        let tick = self.host_state.current_tick;
        match self.executed.iter_mut().find(|t| t.node_id == node_id) {
            Some(trace) => {
                trace.inputs = inputs;
                trace.outputs = outputs;
                for pin in fired {
                    if !trace.fired.contains(&pin) {
                        trace.fired.push(pin);
                    }
                }
            }
            None => self.executed.push(NodeTrace { node_id, tick, inputs, outputs, fired }),
        }
    }
}

fn trace_inputs(inputs: &HashMap<PinId, Option<DataValue>>) -> Vec<(PinId, Option<DataValue>)> {
    let mut traced: Vec<_> = inputs.iter().map(|(p, v)| (*p, v.clone())).collect();
    traced.sort_by_key(|(p, _)| p.0);
    traced
}
