//! Custom plugin data types: engine registration.

use ticknode_shared::*;

use crate::TickEngine;

pub(crate) fn field_pin(
    index: usize,
    field: &FieldDefinition,
    direction: PinDirection,
) -> PinDefinition {
    PinDefinition {
        id: PinId(index as u32 + 1),
        name: if field.title.is_empty() {
            field.name.clone()
        } else {
            field.title.clone()
        },
        pin_type: PinType::Data(field.data_type.clone()),
        direction,
        // Required field — required Make input
        required: direction == PinDirection::Input && !field.optional,
    }
}

impl TickEngine {
    /// Registers plugin types. `nodes` are the node definitions of the same plugin:
    /// their pins may only reference known types. All-or-nothing.
    pub fn register_types(
        &mut self,
        defs: &[TypeDefinition],
        nodes: &[NodeDefinition],
    ) -> Result<(), String> {
        self.register_types_owned(None, defs, nodes)
    }

    /// Same on behalf of plugin `owner`: types are unregistered along with it.
    pub fn register_types_owned(
        &mut self,
        owner: Option<&str>,
        defs: &[TypeDefinition],
        nodes: &[NodeDefinition],
    ) -> Result<(), String> {
        let mut types = self.types.clone();
        types.insert_all(defs)?;
        check_node_types(&types, nodes)?;

        if let Some(owner) = owner {
            let mut owned = types.definitions();
            for def in &mut owned {
                if defs.iter().any(|d| d.name == def.name) {
                    def.plugin = Some(owner.into());
                }
            }
            types = TypeRegistry::from_definitions(owned);
        }
        self.types = types;
        self.refresh_identifier_labels();
        self.notify_types_changed();
        if let Some(owner) = owner {
            self.type_owners
                .extend(defs.iter().map(|d| (d.name.clone(), owner.to_string())));
        }
        Ok(())
    }
}

fn check_node_types(types: &TypeRegistry, nodes: &[NodeDefinition]) -> Result<(), String> {
    for node in nodes {
        for pin in &node.pins {
            if let PinType::Data(t) = &pin.pin_type {
                types
                    .check_known(t)
                    .map_err(|e| format!("Node {}, pin {}: {e}", node.type_name, pin.name))?;
            }
        }
    }
    Ok(())
}
