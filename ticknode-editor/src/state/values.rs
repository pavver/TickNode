//! Typed reads for bound inputs and template preview, using sheet telemetry only.
use super::*;
use ticknode_shared::{effective_type, plugin_values::project};

impl Editor {
    pub(crate) fn plugin_value(&self, path: &str) -> Option<(DataValue, DataType)> {
        self.plugin_values.with(|values| {
            let root = values
                .iter()
                .filter(|v| path == v.path || path.starts_with(&format!("{}.", v.path)))
                .max_by_key(|v| v.path.len())?;
            let value = root.value.as_ref()?;
            let suffix = path.strip_prefix(&root.path)?.trim_start_matches('.');
            let (namespace, key) = root.path.split_once('.')?;
            let declared = self.value_entry_fields.with(|cache| {
                cache
                    .get(&(namespace.into(), key.into()))?
                    .iter()
                    .find(|f| f.path == suffix)
                    .map(|f| f.data_type.clone())
            });
            let ty = self.types.with(|types| {
                let tree = effective_type(Some(value), &DataType::Object, suffix, types)?;
                let declared = declared
                    .filter(|ty| !matches!(ty, DataType::Any | DataType::Object))
                    .unwrap_or(tree);
                effective_type(Some(project(value, suffix)?), &declared, "", types)
            })?;
            Some((project(value, suffix)?.clone(), ty))
        })
    }

    pub(crate) fn bound_reference(&self, node: NodeId, pin: PinId) -> Option<VariableRef> {
        self.node_tracked(node)?.instance.with(|i| {
            i.model
                .properties
                .iter()
                .find(|(name, _)| *name == input_variable_property(pin))
                .and_then(|(_, v)| {
                    if let DataValue::String(s) = v {
                        VariableRef::parse(s)
                    } else {
                        None
                    }
                })
        })
    }

    pub(crate) fn bound_input_type(&self, node: NodeId, pin: PinId) -> Option<DataType> {
        self.reference_type(&self.bound_reference(node, pin)?)
    }

    pub(crate) fn bound_input(&self, node: NodeId, pin: PinId) -> Option<(DataValue, DataType)> {
        self.variable_value(&self.bound_reference(node, pin)?)
    }

    pub(crate) fn variable_value(&self, reference: &VariableRef) -> Option<(DataValue, DataType)> {
        if reference.lookup == VariableLookup::Plugin {
            return self.plugin_value(&reference.name);
        }
        self.variables.with(|variables| {
            self.types.with(|types| {
                let (value, ty) = crate::inline::variable::resolve_variable_value(
                    reference,
                    self.open_sheet.get(),
                    variables,
                    types,
                )?;
                Some((value.clone(), ty))
            })
        })
    }

    /// Formula results use the same live reference contracts as engine evaluation and preview.
    pub(crate) fn output_type(&self, node: NodeId, pin: PinId) -> Option<DataType> {
        let instance = self.node_tracked(node)?.instance.get();
        let declared = instance.definition.pins.iter().find_map(|p| {
            if p.id != pin || p.direction != PinDirection::Output {
                return None;
            }
            if let PinType::Data(ty) = &p.pin_type {
                Some(ty.clone())
            } else {
                None
            }
        })?;
        if instance.definition.type_name != "math/expression" || pin != PinId(0) {
            return Some(declared);
        }
        let property = |name: &str| {
            instance.model.properties.iter().find_map(|(n, v)| {
                if n == name
                    && let DataValue::String(s) = v
                {
                    Some(s.as_str())
                } else {
                    None
                }
            })
        };
        if property("value_type").is_some_and(|v| v != "auto") {
            return Some(declared);
        }
        let Some(compiled) = property("expression").and_then(|s| formula::compile(s).ok()) else {
            return Some(declared);
        };
        let mut types = HashMap::new();
        for input in instance
            .definition
            .pins
            .iter()
            .filter(|p| p.direction == PinDirection::Input)
        {
            let PinType::Data(ty) = &input.pin_type else {
                continue;
            };
            // Read the neighboring declaration without recursing through potentially cyclic wires.
            let source = self.data.with(|wires| {
                wires
                    .iter()
                    .find(|c| c.dst_node == node && c.dst_pin == input.id)
                    .cloned()
            });
            let wired = source.and_then(|c| {
                self.node_tracked(c.src_node)?.instance.with(|i| {
                    i.definition.pins.iter().find_map(|p| {
                        if p.id == c.src_pin
                            && p.direction == PinDirection::Output
                            && let PinType::Data(ty) = &p.pin_type
                        {
                            Some(ty.clone())
                        } else {
                            None
                        }
                    })
                })
            });
            types.insert(
                input.name.clone(),
                wired
                    .or_else(|| self.bound_input_type(node, input.id))
                    .unwrap_or_else(|| ty.clone()),
            );
        }
        for reference in compiled.references() {
            if let Some(ty) = self.reference_type(&reference) {
                types.insert(reference.to_str(), ty);
            }
        }
        Some(compiled.infer_type(&types).unwrap_or(declared))
    }

    pub(crate) fn template_values(&self) -> Vec<(String, Option<DataValue>, DataType)> {
        self.plugin_values.with(|values| {
            self.value_entry_fields.with(|cache| {
                let mut result = Vec::new();
                for value in values {
                    result.push((value.path.clone(), value.value.clone(), DataType::Object));
                    let Some((namespace, key)) = value.path.split_once('.') else {
                        continue;
                    };
                    if let Some(fields) = cache.get(&(namespace.into(), key.into())) {
                        for field in fields {
                            result.push((
                                format!("{}.{}", value.path, field.path),
                                value
                                    .value
                                    .as_ref()
                                    .and_then(|v| project(v, &field.path))
                                    .cloned(),
                                field.data_type.clone(),
                            ));
                        }
                    }
                }
                result
            })
        })
    }
}

#[cfg(test)]
mod tests;
