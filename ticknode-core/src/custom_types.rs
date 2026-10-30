//! Custom plugin data types: engine registration and auto-generated `Make <Type>` / `Break <Type>` nodes.
//!
//! For each struct type, the engine automatically registers two pure nodes: `type/make/<name>` constructs a value from fields,
//! and `type/break/<name>` decomposes a value into fields. Pins are stable: in Make, the output is pin 0 and fields are 1..=N;
//! in Break, the input is pin 0 and fields are 1..=N (in declaration order; append new fields at the end).

use std::collections::BTreeMap;
use std::sync::Arc;

use ticknode_shared::*;

use crate::{Node, NodeContext, TickEngine};

pub fn make_node_type(type_name: &str) -> String {
    format!("type/make/{type_name}")
}

pub fn break_node_type(type_name: &str) -> String {
    format!("type/break/{type_name}")
}

fn field_pin(index: usize, field: &FieldDefinition, direction: PinDirection) -> PinDefinition {
    PinDefinition {
        id: PinId(index as u32 + 1),
        name: if field.title.is_empty() { field.name.clone() } else { field.title.clone() },
        pin_type: PinType::Data(field.data_type.clone()),
        direction,
        // Required field — required Make input
        required: direction == PinDirection::Input && !field.optional,
    }
}

/// `Make <Type>`: fields → typed value.
pub struct TypeMakeNode {
    id: NodeId,
    def: Arc<TypeDefinition>,
}

impl Node for TypeMakeNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }

    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: self.def.title.clone(),
            pin_type: PinType::Data(DataType::Named(self.def.name.clone())),
            direction: PinDirection::Output,
            required: false,
        }];
        pins.extend(self.def.fields().iter().enumerate().map(|(i, f)| field_pin(i, f, PinDirection::Input)));
        NodeDefinition {
            type_name: make_node_type(&self.def.name),
            title: format!("Make {}", self.def.title),
            category: "Data".into(),
            description: format!("Збирає значення «{}» з полів", self.def.title),
            help: format!(
                "Збирає значення типу «{}» ({}) з окремих полів.\n\nКожне поле — вхід: під'єднайте джерело або задайте значення прямо на ноді. \
                 Обов'язкові поля — обов'язкові входи: без них значення не створюється (у лозі ноди з'явиться помилка). Необов'язкове поле без значення пропускається.\n\nЧиста нода: обчислюється щоразу, коли результат потрібен іншій ноді.\n\n{}",
                self.def.title, self.def.name, self.def.description
            ).trim_end().to_string(),
            pins,
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let mut fields = BTreeMap::new();
        for (i, f) in self.def.fields().iter().enumerate() {
            // The engine verified required inputs before execution; an optional field without a value is skipped
            if let Some(value) = ctx.get_value(PinId(i as u32 + 1)) {
                fields.insert(f.name.clone(), ObjectField { name: f.name.clone(), description: f.description.clone(), value });
            } else if !f.optional {
                return Vec::new();
            }
        }
        vec![NodeOutput::Data(PinId(0), DataValue::Object { fields, type_name: Some(self.def.name.clone()) })]
    }
}

/// `Break <Type>`: typed value → fields.
pub struct TypeBreakNode {
    id: NodeId,
    def: Arc<TypeDefinition>,
}

impl Node for TypeBreakNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }

    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: self.def.title.clone(),
            pin_type: PinType::Data(DataType::Named(self.def.name.clone())),
            direction: PinDirection::Input,
            required: false,
        }];
        pins.extend(self.def.fields().iter().enumerate().map(|(i, f)| field_pin(i, f, PinDirection::Output)));
        NodeDefinition {
            type_name: break_node_type(&self.def.name),
            title: format!("Break {}", self.def.title),
            category: "Data".into(),
            description: format!("Розбирає «{}» на поля", self.def.title),
            help: format!(
                "Розбирає значення типу «{}» ({}) на окремі поля: кожне поле — вихідний пін.\n\n\
                 Необов'язкове поле, якого у значенні немає, не має значення (пін порожній).\n\nЧиста нода: обчислюється щоразу, коли результат потрібен іншій ноді.\n\n{}",
                self.def.title, self.def.name, self.def.description
            ).trim_end().to_string(),
            pins,
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let Some(DataValue::Object { fields, .. }) = ctx.get_value(PinId(0)) else { return Vec::new() };
        self.def.fields().iter().enumerate()
            .filter_map(|(i, f)| Some(NodeOutput::Data(PinId(i as u32 + 1), fields.get(&f.name)?.value.clone())))
            .collect()
    }
}

impl TickEngine {
    /// Registers plugin types and Make/Break nodes for its structs. `nodes` are the node definitions
    /// of the same plugin: their pins may only reference known types. All-or-nothing.
    pub fn register_types(&mut self, defs: &[TypeDefinition], nodes: &[NodeDefinition]) -> Result<(), String> {
        self.register_types_owned(None, defs, nodes)
    }

    /// Same on behalf of plugin `owner`: types and Make/Break nodes are unregistered along with it.
    pub fn register_types_owned(&mut self, owner: Option<&str>, defs: &[TypeDefinition], nodes: &[NodeDefinition]) -> Result<(), String> {
        let mut types = self.types.clone();
        types.insert_all(defs)?;
        for node in nodes {
            for pin in &node.pins {
                if let PinType::Data(t) = &pin.pin_type {
                    types.check_known(t).map_err(|e| format!("Нода {}, пін {}: {e}", node.type_name, pin.name))?;
                }
            }
        }
        let generated: Vec<String> = defs.iter().filter(|d| d.is_struct())
            .flat_map(|d| [make_node_type(&d.name), break_node_type(&d.name)])
            .collect();
        for name in &generated {
            if self.registry.get_definition(name).is_some() {
                return Err(format!("Тип ноди {name} уже зареєстровано"));
            }
        }
        if let Some(clash) = nodes.iter().find(|n| generated.contains(&n.type_name)) {
            return Err(format!("Тип ноди {} зарезервовано для автоматичних нод структур", clash.type_name));
        }

        self.types = types;
        if let Some(owner) = owner {
            self.type_owners.extend(defs.iter().map(|d| (d.name.clone(), owner.to_string())));
        }
        let register = |registry: &mut crate::NodeRegistry, creator: crate::NodeCreator| match owner {
            Some(owner) => registry.register_owned(owner, creator),
            None => registry.register(creator),
        };
        for def in defs.iter().filter(|d| d.is_struct()) {
            let def = Arc::new(def.clone());
            let make_def = def.clone();
            register(&mut self.registry, Box::new(move |id| Box::new(TypeMakeNode { id, def: make_def.clone() })));
            register(&mut self.registry, Box::new(move |id| Box::new(TypeBreakNode { id, def: def.clone() })));
        }
        Ok(())
    }
}
