//! Generic struct pin projection and assembly. Node implementations only see their original pins.
use std::collections::{BTreeMap, HashMap};
use ticknode_shared::*;
use crate::{EngineError, RemovedConnections, TickEngine};

impl TickEngine {
    fn pin_is_split(&self, node: NodeId, pin: PinId) -> bool {
        self.split_pins.get(&node).is_some_and(|pins| pins.contains(&pin.0))
    }

    pub(crate) fn effective_definition(&self, node: NodeId, mut def: NodeDefinition) -> NodeDefinition {
        // Stubs already retain the last effective definition, even after their types disappear.
        if self.node_unavailable(node).is_some() { return def; }
        def.pins = def.pins.into_iter().filter(|pin| !is_virtual_pin(pin.id)).flat_map(|pin| {
            if self.pin_is_split(node, pin.id) && let Some(t) = splittable_type(&self.types, &pin) {
                return t.fields().iter().enumerate().map(|(index, field)| PinDefinition {
                    id: split_field_pin(pin.id, index).expect("splittable type"),
                    name: format!("{}.{}", pin.name, if field.title.is_empty() { &field.name } else { &field.title }),
                    pin_type: PinType::Data(field.data_type.clone()),
                    direction: pin.direction,
                    required: pin.direction == PinDirection::Input && !field.optional,
                }).collect::<Vec<_>>();
            }
            vec![pin]
        }).collect();
        def
    }

    pub fn set_pin_split(&mut self, node: NodeId, pin: PinId, split: bool) -> Result<RemovedConnections, EngineError> {
        let n = self.nodes.get(&node).ok_or(EngineError::NodeNotFound(node))?;
        if let Some(reason) = n.unavailable() { return Err(EngineError::PluginUnavailable(reason)); }
        // A property addresses an id, so struct pins sharing it in both directions split together.
        // Other pins sharing the id (e.g. an exec pin) remain unchanged.
        let def = n.definition();
        let pins: Vec<_> = def.pins.iter().filter(|p| p.id == pin).collect();
        if !pins.iter().any(|p| splittable_type(&self.types, p).is_some()) {
            return Err(EngineError::InvalidPinConnection);
        }
        let set = self.split_pins.entry(node).or_default();
        if split { set.insert(pin.0); } else { set.remove(&pin.0); }
        self.invalidate_definition(node);
        self.pins_edited(node);
        self.pure_cache.clear();
        self.required_reported.remove(&node);
        let effective = self.node_definition(node).expect("node exists");
        if let Some(values) = self.input_values.get_mut(&node) {
            values.retain(|id, value| super::input_pin_type(&effective, PinId(*id))
                .is_some_and(|t| input_value_fits(&self.types, &t, value)));
        }
        Ok(self.prune_connections(|c| c.src_node == node || c.dst_node == node))
    }

    pub(crate) fn split_output_field(&self, node: NodeId, pin: PinId) -> Option<(PinId, String)> {
        let (parent, index) = split_field_parent(pin)?;
        if !self.pin_is_split(node, parent) || self.node_unavailable(node).is_some() { return None; }
        let def = self.nodes.get(&node)?.definition();
        let pin = def.pins.iter().find(|p| p.id == parent && p.direction == PinDirection::Output)?;
        let t = splittable_type(&self.types, pin)?;
        Some((parent, t.fields().get(index)?.name.clone()))
    }

    pub(crate) fn assemble_split_inputs(&self, node: NodeId, inputs: &mut HashMap<PinId, Option<DataValue>>) {
        if self.split_pins.get(&node).is_none_or(|pins| pins.is_empty()) { return; }
        let Some(n) = self.nodes.get(&node) else { return };
        for pin in n.definition().pins.iter().filter(|p| p.direction == PinDirection::Input && self.pin_is_split(node, p.id)) {
            let Some(t) = splittable_type(&self.types, pin) else { continue };
            let mut fields = BTreeMap::new();
            let mut complete = true;
            for (index, field) in t.fields().iter().enumerate() {
                let id = split_field_pin(pin.id, index).expect("splittable type");
                if let Some(Some(value)) = inputs.get(&id) {
                    fields.insert(field.name.clone(), ObjectField {
                        name: field.name.clone(), description: field.description.clone(), value: value.clone(),
                    });
                } else if !field.optional { complete = false; }
            }
            inputs.insert(pin.id, complete.then(|| DataValue::Object { fields, type_name: Some(t.name.clone()) }));
        }
    }

    pub(crate) fn native_inputs(&self, node: NodeId, mut inputs: HashMap<PinId, Option<DataValue>>) -> HashMap<PinId, Option<DataValue>> {
        if self.node_unavailable(node).is_none() {
            inputs.retain(|pin, _| !is_virtual_pin(*pin));
        }
        inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> TickEngine {
        let mut engine = TickEngine::new();
        engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
            FieldDefinition::new("x", "X", DataType::Number),
            FieldDefinition::new("label", "Label", DataType::String).optional(),
        ])], &[]).unwrap();
        engine
    }

    fn make(engine: &mut TickEngine) -> NodeId {
        let id = NodeId::generate();
        engine.create_node(id, "type/make/test.point", &[]).unwrap();
        engine.set_input_value(id, PinId(1), Some(42i64.into())).unwrap();
        id
    }

    #[test]
    fn pure_output_fields_optional_clear_and_collapse() {
        let mut e = engine();
        let source = make(&mut e);
        let sink = NodeId::generate();
        e.create_node(sink, "type/break/test.point", &[]).unwrap();
        e.connect_data(source, PinId(0), sink, PinId(0)).unwrap();
        assert_eq!(e.set_pin_split(source, PinId(0), true).unwrap().data.len(), 1);
        let x = split_field_pin(PinId(0), 0).unwrap();
        let label = split_field_pin(PinId(0), 1).unwrap();
        assert_eq!(e.pull_data(source, x, &mut vec![]), Some(42i64.into()));
        assert_eq!(e.pull_data(source, label, &mut vec![]), None);
        e.set_input_value(source, PinId(2), Some("hello".into())).unwrap();
        assert_eq!(e.pull_data(source, label, &mut vec![]), Some("hello".into()));
        e.set_input_value(source, PinId(2), None).unwrap();
        assert_eq!(e.pull_data(source, label, &mut vec![]), None);
        e.set_pin_split(sink, PinId(0), true).unwrap();
        e.connect_data(source, label, sink, x).unwrap();
        assert_eq!(e.pull_data(sink, PinId(1), &mut vec![]), None);
        assert!(!e.host_state.logs.iter().any(|(id, _)| *id == sink), "connected missing fields skip required consumers silently");
        e.connect_data(source, x, sink, x).unwrap();
        assert_eq!(e.pull_data(sink, PinId(1), &mut vec![]), Some(42i64.into()));
        assert_eq!(e.set_pin_split(source, PinId(0), false).unwrap().data.len(), 1);
        assert!(e.connect_data(source, x, sink, x).is_err());
    }

    #[test]
    fn input_assembly_tag_required_fields_and_persistence() {
        let mut e = engine();
        let id = NodeId::generate();
        e.create_node(id, "type/break/test.point", &[]).unwrap();
        e.set_pin_split(id, PinId(0), true).unwrap();
        let x = split_field_pin(PinId(0), 0).unwrap();
        let label = split_field_pin(PinId(0), 1).unwrap();
        assert_eq!(e.pull_data(id, PinId(1), &mut vec![]), None);
        assert!(e.host_state.logs.iter().any(|(_, msg)| msg.contains("Required input")));
        e.set_input_value(id, x, Some(7i64.into())).unwrap();
        let inputs = e.pull_inputs(id, &mut vec![]);
        let Some(Some(DataValue::Object { fields, type_name })) = inputs.get(&PinId(0)) else { panic!("missing struct") };
        assert_eq!(type_name.as_deref(), Some("test.point"));
        assert_eq!(fields.len(), 1);
        assert_eq!(e.native_inputs(id, inputs).len(), 1, "virtual pins do not reach node implementations");
        assert_eq!(e.pull_data(id, PinId(1), &mut vec![]), Some(7i64.into()));
        e.set_input_value(id, label, Some("copy".into())).unwrap();
        let graph = e.export_graph();
        let mut restored = engine();
        assert!(restored.load_graph(&graph).is_empty());
        assert_eq!(restored.export_graph(), graph);
        assert_eq!(restored.pull_data(id, PinId(2), &mut vec![]), Some("copy".into()));
        let (map, warnings) = restored.import_snippet(&e.export_snippet(&[id]));
        assert!(warnings.is_empty());
        assert_eq!(restored.pull_data(map[0].1, PinId(1), &mut vec![]), Some(7i64.into()));
        restored.set_pin_split(id, PinId(0), false).unwrap();
        assert!(restored.input_values.get(&id).unwrap().is_empty());
        assert!(!restored.node_model(id).unwrap().properties.iter().any(|(k, _)| k.starts_with("split:")));
        // An explicitly optional original input still cannot build a partial struct.
        restored.set_pin_split(id, PinId(0), true).unwrap();
        assert_eq!(restored.pull_inputs(id, &mut vec![])[&PinId(0)], None);
    }

    #[test]
    fn dynamic_pin_changes_prune_fields_and_inline_values() {
        let mut e = engine();
        let source = make(&mut e);
        e.set_pin_split(source, PinId(0), true).unwrap();
        let x = split_field_pin(PinId(0), 0).unwrap();
        let sink = NodeId::generate();
        e.create_node(sink, "object/set_field", &[("field_type".into(), "test.point".into())]).unwrap();
        let def = e.nodes[&sink].definition();
        let pin = def.pins.iter().find(|p| p.pin_type == PinType::Data(DataType::Named("test.point".into()))).unwrap().id;
        e.set_pin_split(sink, pin, true).unwrap();
        let field = split_field_pin(pin, 0).unwrap();
        e.set_input_value(sink, field, Some(3i64.into())).unwrap();
        e.connect_data(source, x, sink, field).unwrap();
        assert_eq!(e.set_node_property(sink, "field_type", "number".into()).unwrap().data.len(), 1);
        assert!(e.input_values[&sink].is_empty());
    }
}
