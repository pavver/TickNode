use super::*;
use crate::VariableError;

struct VariableOutputs<'a> {
    node: NodeId,
    sheet: Option<SheetId>,
    fired: &'a mut Vec<PinId>,
    queue: &'a mut VecDeque<(NodeId, PinId)>,
}

impl TickEngine {
    fn variable_input_type(&mut self, id: NodeId) -> DataType {
        self.input_source_types(id)
            .remove(&PinId(3))
            .or_else(|| {
                self.node_definition(id)?
                    .pins
                    .into_iter()
                    .find(|p| p.id == PinId(3) && p.direction == PinDirection::Input)
                    .and_then(|p| match p.pin_type {
                        PinType::Data(t) => Some(t),
                        _ => None,
                    })
            })
            .unwrap_or(DataType::Any)
    }

    /// Exec outputs are queued for the next tick; data and variables are applied immediately.
    pub(crate) fn apply_outputs(
        &mut self,
        node_id: NodeId,
        inputs: Vec<(PinId, Option<DataValue>)>,
        outputs: Vec<NodeOutput>,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) {
        self.sync_plugin_values();
        let mut state_changed = false;
        let mut fired = Vec::new();
        for output in outputs {
            match output {
                NodeOutput::Execution(out_exec_pin) => {
                    fired.push(out_exec_pin);
                    self.queue_exec_output(node_id, out_exec_pin, next_tick_queue);
                }
                NodeOutput::Data(pin_id, value) => {
                    self.data_cache
                        .entry(node_id)
                        .or_default()
                        .insert(pin_id, value);
                    state_changed = true;
                }
                NodeOutput::Clear(pin_id) => {
                    if let Some(values) = self.data_cache.get_mut(&node_id) {
                        values.remove(&pin_id);
                    }
                    state_changed = true;
                }
                NodeOutput::Variable(cmd) => {
                    if self.apply_variable_command(node_id, cmd, &mut fired, next_tick_queue) {
                        state_changed = true;
                    }
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
    pub(super) fn record_trace(
        &mut self,
        node_id: NodeId,
        inputs: Vec<(PinId, Option<DataValue>)>,
        fired: Vec<PinId>,
    ) {
        self.successful.insert(node_id);
        let mut outputs: Vec<(PinId, DataValue)> = self
            .data_cache
            .get(&node_id)
            .into_iter()
            .flatten()
            .chain(self.pure_cache.get(&node_id).into_iter().flatten())
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        self.project_trace_outputs(node_id, &mut outputs);
        outputs.sort_by_key(|(p, _)| p.0);
        outputs.dedup_by_key(|(p, _)| *p);
        let tick = self.host_state.current_tick;
        let slot = self
            .executed_index
            .get(&node_id)
            .copied()
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
                self.executed.push(NodeTrace {
                    node_id,
                    tick,
                    inputs,
                    outputs,
                    fired,
                });
            }
        }
    }
    fn queue_exec_output(
        &self,
        node_id: NodeId,
        out_exec_pin: PinId,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) {
        for c in self.connections.exec_successors(node_id, out_exec_pin) {
            next_tick_queue.push_back((c.dst_node, c.dst_pin));
        }
    }

    fn project_trace_outputs(&self, node_id: NodeId, outputs: &mut Vec<(PinId, DataValue)>) {
        // Telemetry uses the same visible pins as the editor, without evaluating any node again.
        if let Some(def) = self.node_definition(node_id) {
            let projected: Vec<_> = def
                .pins
                .iter()
                .filter(|p| p.direction == PinDirection::Output)
                .filter_map(|p| {
                    let (parent, field) = self.split_output_field(node_id, p.id)?;
                    let DataValue::Object { fields, .. } =
                        &outputs.iter().find(|(id, _)| *id == parent)?.1
                    else {
                        return None;
                    };
                    Some((p.id, fields.get(&field)?.value.clone()))
                })
                .collect();
            outputs.retain(|(id, _)| {
                def.pins
                    .iter()
                    .any(|p| p.id == *id && p.direction == PinDirection::Output)
            });
            outputs.extend(projected);
        }
    }

    fn resolve_create_key(
        &mut self,
        node_id: NodeId,
        name: &str,
        scope: VariableScope,
        sheet: Option<SheetId>,
    ) -> Option<VariableKey> {
        if !valid_variable_name(name) {
            let err = VariableError::InvalidName(name.to_string());
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            return None;
        }
        match scope {
            VariableScope::Sheet => {
                let Some(s) = sheet else {
                    let err = VariableError::InvalidName(name.to_string());
                    self.host_state
                        .error(node_id, err.ui_message(), err.log_message());
                    return None;
                };
                Some(VariableKey::sheet(s, name))
            }
            VariableScope::Global => Some(VariableKey::global(name)),
            VariableScope::Persistent => Some(VariableKey::persistent(name)),
        }
    }

    fn apply_variable_create(
        &mut self,
        name: String,
        scope: VariableScope,
        data_type: DataType,
        value: DataValue,
        output: &mut VariableOutputs<'_>,
    ) -> bool {
        let node_id = output.node;
        let sheet = output.sheet;
        let Some(key) = self.resolve_create_key(node_id, &name, scope, sheet) else {
            return false;
        };
        if self
            .variables
            .exists_in_scope_namespace(&name, scope, sheet)
        {
            output.fired.push(PinId(2));
            self.queue_exec_output(node_id, PinId(2), output.queue);
            return false;
        }
        if self.variables.len() >= MAX_VARIABLES {
            let err = VariableError::LimitReached;
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            return false;
        }
        let source_type = self.variable_input_type(node_id);
        let Some(converted) = fit_typed_variable_value(
            &self.types,
            value,
            &source_type,
            &data_type,
            &self.host_state.display,
            crate::quantity::labels(&self.host_state.locale),
        ) else {
            let err = VariableError::TypeMismatch(name);
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            return false;
        };
        if !valid_variable_value(&converted) {
            let err = VariableError::LimitReached;
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            return false;
        }
        self.variables.create(key.clone(), data_type, converted);
        self.current_tick_changed_variables.insert(key);
        output.fired.push(PinId(1));
        self.queue_exec_output(node_id, PinId(1), output.queue);
        true
    }

    fn apply_variable_set(
        &mut self,
        name: String,
        lookup: VariableLookup,
        value: DataValue,
        output: &mut VariableOutputs<'_>,
    ) -> bool {
        let node_id = output.node;
        let sheet = output.sheet;
        if !valid_variable_name(&name) {
            let err = VariableError::InvalidName(name);
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            output.fired.push(PinId(2));
            self.queue_exec_output(node_id, PinId(2), output.queue);
            return false;
        }
        let Some(key) = self.variables.resolve(&name, lookup, sheet) else {
            let err = VariableError::Missing(name);
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            output.fired.push(PinId(2));
            self.queue_exec_output(node_id, PinId(2), output.queue);
            return false;
        };
        let Some(var) = self.variables.get(&key) else {
            return false;
        };
        let var_type = var.data_type.clone();
        let source_type = self.variable_input_type(node_id);
        let Some(converted) = fit_typed_variable_value(
            &self.types,
            value,
            &source_type,
            &var_type,
            &self.host_state.display,
            crate::quantity::labels(&self.host_state.locale),
        ) else {
            let err = VariableError::TypeMismatch(name);
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            output.fired.push(PinId(2));
            self.queue_exec_output(node_id, PinId(2), output.queue);
            return false;
        };
        if !valid_variable_value(&converted) {
            let err = VariableError::LimitReached;
            self.host_state
                .error(node_id, err.ui_message(), err.log_message());
            output.fired.push(PinId(2));
            self.queue_exec_output(node_id, PinId(2), output.queue);
            return false;
        }
        let changed = self.variables.set(&key, converted);
        if changed {
            self.current_tick_changed_variables.insert(key);
        }
        output.fired.push(PinId(1));
        self.queue_exec_output(node_id, PinId(1), output.queue);
        true
    }

    fn apply_variable_delete(
        &mut self,
        name: String,
        lookup: VariableLookup,
        output: &mut VariableOutputs<'_>,
    ) -> bool {
        let node_id = output.node;
        let sheet = output.sheet;
        let mut changed = false;
        if valid_variable_name(&name)
            && let Some(key) = self.variables.resolve(&name, lookup, sheet)
        {
            changed = self.variables.delete(&key);
        }
        output.fired.push(PinId(1));
        self.queue_exec_output(node_id, PinId(1), output.queue);
        changed
    }

    fn apply_variable_command(
        &mut self,
        node_id: NodeId,
        cmd: VariableCommand,
        fired: &mut Vec<PinId>,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) -> bool {
        let mut output = VariableOutputs {
            node: node_id,
            sheet: self.node_sheets.get(&node_id).copied(),
            fired,
            queue: next_tick_queue,
        };
        match cmd {
            VariableCommand::Create {
                name,
                scope,
                data_type,
                value,
            } => self.apply_variable_create(name, scope, data_type, value, &mut output),
            VariableCommand::Set {
                name,
                lookup,
                value,
            } => self.apply_variable_set(name, lookup, value, &mut output),
            VariableCommand::Delete { name, lookup } => {
                self.apply_variable_delete(name, lookup, &mut output)
            }
        }
    }
}
