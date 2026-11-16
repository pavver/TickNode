use super::*;

impl TickEngine {
    pub(super) fn input_source_types(&mut self, node_id: NodeId) -> HashMap<PinId, DataType> {
        let mut result: HashMap<PinId, DataType> = self
            .connections
            .data_into(node_id)
            .to_vec()
            .into_iter()
            .filter_map(|c| {
                let source = self.definition_arc(c.src_node)?;
                let pin = source
                    .pins
                    .iter()
                    .find(|p| p.id == c.src_pin && p.direction == PinDirection::Output)?;
                let PinType::Data(ty) = &pin.pin_type else {
                    return None;
                };
                Some((c.dst_pin, ty.clone()))
            })
            .collect();
        for (pin, reference) in self.input_variables.get(&node_id).into_iter().flatten() {
            let ty = if reference.lookup == VariableLookup::Plugin {
                self.plugin_values
                    .resolve(&reference.name)
                    .map(|(_, ty)| ty)
            } else {
                self.variables
                    .resolve(
                        &reference.name,
                        reference.lookup,
                        self.node_sheets.get(&node_id).copied(),
                    )
                    .and_then(|key| self.variables.get(&key))
                    .map(|var| var.data_type.clone())
            };
            if let Some(ty) = ty {
                result.entry(PinId(*pin)).or_insert(ty);
            }
        }
        result
    }

    pub(crate) fn pull_data(
        &mut self,
        node_id: NodeId,
        pin_id: PinId,
        stack: &mut Vec<NodeId>,
    ) -> Option<DataValue> {
        self.sync_plugin_values();
        if self.is_disabled(node_id) || self.skip_frozen(node_id) {
            return None;
        }
        if stack.contains(&node_id) {
            return None;
        }
        if let Some((parent, field)) = self.split_output_field(node_id, pin_id) {
            let DataValue::Object { fields, .. } = self.pull_data(node_id, parent, stack)? else {
                return None;
            };
            return fields.get(&field).map(|f| f.value.clone());
        }

        // Check whether this is an input pin connected to another node's output.
        // If so, recursively pull data from the source.
        let incoming = self.connections.data_source(node_id, pin_id);
        if let Some(c) = incoming {
            stack.push(node_id);
            let val = self.pull_data(c.src_node, c.src_pin, stack);
            stack.pop();
            return val;
        }

        if let Some(cached_node_vals) = self.pure_cache.get(&node_id) {
            return cached_node_vals.get(&pin_id).cloned();
        }

        let is_pure = self
            .nodes
            .get(&node_id)
            .map(|n| n.is_pure())
            .unwrap_or(false);
        if is_pure {
            self.pull_pure(node_id, pin_id, stack)
        } else {
            self.data_cache
                .get(&node_id)
                .and_then(|m| m.get(&pin_id))
                .cloned()
        }
    }

    pub(crate) fn pull_inputs(
        &mut self,
        node_id: NodeId,
        stack: &mut Vec<NodeId>,
    ) -> HashMap<PinId, Option<DataValue>> {
        let connections = self.connections.data_into(node_id).to_vec();
        let mut inputs: HashMap<PinId, Option<DataValue>> = connections
            .into_iter()
            .map(|c| {
                let mut value = self.pull_data(c.src_node, c.src_pin, stack);
                let src_def = self.definition_arc(c.src_node);
                let dst_def = self.definition_arc(c.dst_node);
                let src_pin = src_def.as_ref().and_then(|d| {
                    d.pins
                        .iter()
                        .find(|p| p.id == c.src_pin && p.direction == PinDirection::Output)
                });
                let dst_pin = dst_def.as_ref().and_then(|d| {
                    d.pins
                        .iter()
                        .find(|p| p.id == c.dst_pin && p.direction == PinDirection::Input)
                });
                if let (Some(PinType::Data(src)), Some(PinType::Data(dst))) =
                    (src_pin.map(|p| &p.pin_type), dst_pin.map(|p| &p.pin_type))
                    && let Some(conversion) = self.types.convertible(src, dst)
                    && let Some(original) = value.take()
                {
                    value = conversion.apply_with_display(
                        original,
                        &self.host_state.display,
                        crate::quantity::labels(&self.host_state.locale),
                    );
                    if value.is_none()
                        && let Some(pin) = dst_pin.filter(|p| p.required)
                        && self.required_reported.insert(node_id)
                    {
                        self.host_state.error(
                            node_id,
                            UiMessage::new(
                                "error-node-required-conversion",
                                format!(r#"Required input "{}" could not be converted"#, pin.name),
                            )
                            .arg("pin", &pin.name),
                            format!(
                                "[ERROR] Required input \"{}\" could not be converted",
                                pin.name
                            ),
                        );
                    }
                }
                if let Some(PinType::Data(t)) = dst_pin.map(|p| &p.pin_type)
                    && value
                        .as_ref()
                        .is_some_and(|v| !self.types.identifier_value_fits(t, v))
                {
                    value = None;
                }
                (c.dst_pin, value)
            })
            .collect();
        for (pin, value) in self.input_values.get(&node_id).into_iter().flatten() {
            inputs
                .entry(PinId(*pin))
                .or_insert_with(|| Some(value.clone()));
        }
        self.pull_bound_variables(node_id, &mut inputs);
        self.assemble_split_inputs(node_id, &mut inputs);
        inputs
    }

    fn pull_bound_variables(
        &mut self,
        node_id: NodeId,
        inputs: &mut HashMap<PinId, Option<DataValue>>,
    ) {
        let bound_vars: Vec<(u32, VariableRef)> = self
            .input_variables
            .get(&node_id)
            .into_iter()
            .flatten()
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        if !bound_vars.is_empty() {
            let sheet = self.node_sheets.get(&node_id).copied();
            let def = self.definition_arc(node_id);
            for (pin_num, var_ref) in bound_vars {
                let pin = PinId(pin_num);
                if inputs.contains_key(&pin) {
                    continue;
                }
                let pin_def = def.as_ref().and_then(|d| {
                    d.pins
                        .iter()
                        .find(|p| p.id == pin && p.direction == PinDirection::Input)
                });
                let Some(PinType::Data(pin_type)) = pin_def.map(|p| &p.pin_type) else {
                    continue;
                };
                let value = self.bound_variable_value(&var_ref, pin_type, sheet);
                if value.is_none()
                    && let Some(pin_def) = pin_def
                {
                    self.report_missing_variable(node_id, pin_def, &var_ref);
                }
                inputs.insert(pin, value);
            }
        }
    }

    fn bound_variable_value(
        &self,
        var_ref: &VariableRef,
        pin_type: &DataType,
        sheet: Option<SheetId>,
    ) -> Option<DataValue> {
        let (original, data_type) = if var_ref.lookup == VariableLookup::Plugin {
            self.plugin_values.resolve(&var_ref.name)?
        } else {
            let key = self
                .variables
                .resolve(&var_ref.name, var_ref.lookup, sheet)?;
            let var = self.variables.get(&key)?;
            (&var.value, var.data_type.clone())
        };
        let value = if self.types.compatible(&data_type, pin_type) {
            Some(original.clone())
        } else {
            self.types
                .convertible(&data_type, pin_type)?
                .apply_with_display(
                    original.clone(),
                    &self.host_state.display,
                    crate::quantity::labels(&self.host_state.locale),
                )
        };
        value.filter(|v| self.types.identifier_value_fits(pin_type, v))
    }

    fn report_missing_variable(
        &mut self,
        node_id: NodeId,
        p: &PinDefinition,
        var_ref: &VariableRef,
    ) {
        if p.required && self.required_reported.insert(node_id) {
            self.host_state.error(
                node_id,
                UiMessage::new(
                    "error-node-required-variable",
                    format!(
                        r#"Required input "{}" variable "{}" is not available"#,
                        p.name, var_ref
                    ),
                )
                .arg("pin", &p.name)
                .arg("variable", var_ref.to_str()),
                format!(
                    r#"[ERROR] Required input "{}" variable "{}" is not available"#,
                    p.name, var_ref
                ),
            );
        }
    }

    fn pull_pure(
        &mut self,
        node_id: NodeId,
        pin_id: PinId,
        stack: &mut Vec<NodeId>,
    ) -> Option<DataValue> {
        stack.push(node_id);
        let definition = self.definition_arc(node_id)?;
        let inputs = self.pull_inputs(node_id, stack);
        if self.skip_for_required(node_id, &definition, &inputs) {
            stack.pop();
            return None;
        }
        let traced = trace_inputs(&inputs);
        let inputs = self.native_inputs(node_id, inputs);
        let mut node = self.nodes.remove(&node_id)?;
        let sheet = self.node_sheets.get(&node_id).copied();
        let input_types = self.input_source_types(node_id);
        let mut ctx = NodeContext {
            node_id,
            inputs,
            input_types,
            plugin_values: &self.plugin_values,
            variables: &self.variables,
            types: &self.types,
            sheet,
            host_state: &mut self.host_state,
        };
        let outputs = node.execute(None, &mut ctx);
        self.charge_node(node_id, definition.cost);
        self.nodes.insert(node_id, node);
        self.sync_plugin_values();
        stack.pop();

        let node_outputs = self.pure_output_values(node_id, outputs);
        let target_val = node_outputs.get(&pin_id).cloned();
        self.pure_cache.insert(node_id, node_outputs);
        self.record_trace(node_id, traced, Vec::new());
        target_val
    }
    fn pure_output_values(
        &mut self,
        node_id: NodeId,
        outputs: Vec<NodeOutput>,
    ) -> HashMap<PinId, DataValue> {
        let mut node_outputs = HashMap::new();
        let mut had_exec_output = false;
        for out in outputs {
            match out {
                NodeOutput::Data(p, v) => {
                    node_outputs.insert(p, v);
                }
                NodeOutput::Clear(p) => {
                    node_outputs.remove(&p);
                }
                NodeOutput::Execution(pin) => {
                    had_exec_output = true;
                    if self.pure_exec_reported.insert(node_id) {
                        tracing::warn!(
                            node = ?node_id,
                            pin = pin.0,
                            "Pure node execution output is ignored"
                        );
                        self.host_state.log(
                            node_id,
                            format!(
                                "[WARN] Pure node emitted execution output on pin {}: ignored",
                                pin.0
                            ),
                        );
                    }
                }
                _ => {}
            }
        }
        if !had_exec_output {
            self.pure_exec_reported.remove(&node_id);
        }
        node_outputs
    }
}
