use std::collections::HashMap;
use ticknode_shared::*;
use crate::{Node, builtin::*};

pub type NodeCreator = Box<dyn Fn(NodeId) -> Box<dyn Node> + Send + Sync>;

pub struct NodeRegistry {
    definitions: HashMap<String, NodeDefinition>,
    creators: HashMap<String, NodeCreator>,
    /// Which plugin registered the node type (built-in types have no owner).
    owners: HashMap<String, String>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            creators: HashMap::new(),
            owners: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Registers a node type. The definition is taken from an instance created with default properties,
    /// so the registry and the node itself cannot diverge.
    pub fn register(&mut self, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.register_definition(definition, creator);
    }

    /// Registers a node type on behalf of a plugin (so it can be unregistered when the plugin is disabled).
    pub fn register_owned(&mut self, owner: &str, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.owners.insert(definition.type_name.clone(), owner.to_string());
        self.register_definition(definition, creator);
    }

    fn register_definition(&mut self, definition: NodeDefinition, creator: NodeCreator) {
        assert!(definition.pins.iter().all(|p| !is_virtual_pin(p.id)), "Real pins must not use reserved virtual ids");
        let type_name = definition.type_name.clone();
        self.definitions.insert(type_name.clone(), definition);
        self.creators.insert(type_name, creator);
    }

    pub fn owner_of(&self, type_name: &str) -> Option<&str> {
        self.owners.get(type_name).map(String::as_str)
    }

    pub fn types_owned_by(&self, owner: &str) -> Vec<String> {
        self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect()
    }

    /// Unregisters all node types owned by the plugin and returns their names.
    pub fn unregister_owner(&mut self, owner: &str) -> Vec<String> {
        let names: Vec<String> = self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect();
        for name in &names {
            self.owners.remove(name);
            self.definitions.remove(name);
            self.creators.remove(name);
        }
        names
    }

    pub fn get_definitions(&self) -> Vec<NodeDefinition> {
        let mut defs: Vec<_> = self.definitions.values().cloned().collect();
        defs.sort_by(|a, b| (&a.category, &a.title).cmp(&(&b.category, &b.title)));
        defs
    }

    pub fn get_definition(&self, type_name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(type_name)
    }

    pub fn create_node(&self, type_name: &str, id: NodeId) -> Option<Box<dyn Node>> {
        self.creators.get(type_name).map(|f| f(id))
    }

    fn register_defaults(&mut self) {
        self.register(Box::new(|id| Box::new(ClockNode { id, interval_ticks: 10 })));
        self.register(Box::new(|id| Box::new(ConsoleNode { id })));
        self.register(Box::new(|id| Box::new(ConstNode { id, value: DataValue::Number(NumberValue::Int(0)), value_type: None })));
        for op in [MathOp::Add, MathOp::Sub, MathOp::Mul, MathOp::Div] {
            self.register(Box::new(move |id| Box::new(PureMathNode::new(id, op))));
        }
        self.register(Box::new(|id| Box::new(MathExpressionNode {
            id,
            expression: "a + b".to_string(),
            variable_mapping: expression_variables("a + b"),
        })));
        for op in [LogOp::And, LogOp::Or, LogOp::Not] {
            self.register(Box::new(move |id| Box::new(LogicalNode { id, op })));
        }
        self.register(Box::new(|id| Box::new(VariableGetNode::new(id))));
        self.register(Box::new(|id| Box::new(VariableSetNode::new(id))));
        self.register(Box::new(|id| Box::new(VariableCreateNode::new(id))));
        self.register(Box::new(|id| Box::new(VariableDeleteNode::new(id))));
        self.register(Box::new(|id| Box::new(VariableChangedNode::new(id))));
        self.register(Box::new(|id| Box::new(MatchNode::new(id))));
        self.register(Box::new(|id| Box::new(ToTextNode::new(id))));
        self.register(Box::new(|id| Box::new(ConvertNode::new(id))));
        self.register(Box::new(|id| Box::new(CompareNode::new(id))));
        self.register(Box::new(|id| Box::new(HysteresisNode::new(id))));
        self.register(Box::new(|id| Box::new(ForLoopNode { id, active: false, current_index: 0, count: 0 })));
        self.register(Box::new(|id| Box::new(SleepNode { id })));
        self.register(Box::new(|id| Box::new(ListCreateNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListAddNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListRemoveNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListGetNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListLengthNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectCreateNode { id })));
        self.register(Box::new(|id| Box::new(ObjectSetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectGetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectUnpackNode { id, fields_list: String::new(), parsed_fields: Vec::new() })));
        self.register(Box::new(|id| Box::new(ClickButtonNode { id, label: "Button".to_string() })));
        self.register(Box::new(|id| Box::new(DebugNode { id, prefix: String::new() })));
    }
}

impl Default for NodeRegistry {
    fn default() -> Self { Self::new() }
}
