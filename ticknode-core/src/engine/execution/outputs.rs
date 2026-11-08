use super::*;

impl TickEngine {
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
                    self.queue_exec_output(node_id, out_exec_pin, next_tick_queue);
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
    pub(super) fn record_trace(&mut self, node_id: NodeId, inputs: Vec<(PinId, Option<DataValue>)>, fired: Vec<PinId>) {
        let mut outputs: Vec<(PinId, DataValue)> = self.data_cache.get(&node_id).into_iter().flatten()
            .chain(self.pure_cache.get(&node_id).into_iter().flatten())
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        self.project_trace_outputs(node_id, &mut outputs);
        outputs.sort_by_key(|(p, _)| p.0);
        outputs.dedup_by_key(|(p, _)| *p);
        let tick = self.host_state.current_tick;
        let slot = self.executed_index.get(&node_id).copied()
            .filter(|&i| self.executed.get(i).is_some_and(|t| t.node_id == node_id));
        match slot.map(|i| &mut self.executed[i]) {
            Some(trace) => {
                trace.inputs = inputs;
                trace.outputs = outputs;
                for pin in fired {
                    if !trace.fired.contains(&pin) {
                        trace.fired.push(pin);
                    }
                }
            }
            None => {
                self.executed_index.insert(node_id, self.executed.len());
                self.executed.push(NodeTrace { node_id, tick, inputs, outputs, fired });
            }
        }
    }
    fn queue_exec_output(&self, node_id: NodeId, out_exec_pin: PinId, next_tick_queue: &mut VecDeque<(NodeId, PinId)>) {
        for c in self.connections.exec_successors(node_id, out_exec_pin) {
            next_tick_queue.push_back((c.dst_node, c.dst_pin));
        }
    }

    fn project_trace_outputs(&self, node_id: NodeId, outputs: &mut Vec<(PinId, DataValue)>) {
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
    }
}
