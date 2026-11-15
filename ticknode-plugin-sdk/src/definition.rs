use crate::{PinDefinition, PropertyDefinition};

/// Definition of a plugin node exposed to the host and palette.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDefinition {
    pub type_name: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub help: String,
    pub pins: Vec<PinDefinition>,
    pub properties: Vec<PropertyDefinition>,
    pub cost: u32,
    pub adaptive: bool,
    pub pure: bool,
}

impl Default for NodeDefinition {
    fn default() -> Self {
        Self {
            type_name: String::new(),
            title: String::new(),
            category: String::new(),
            description: String::new(),
            help: String::new(),
            pins: Vec::new(),
            properties: Vec::new(),
            cost: 1,
            adaptive: false,
            pure: false,
        }
    }
}

impl NodeDefinition {
    pub fn new(
        type_name: impl Into<String>,
        title: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            title: title.into(),
            category: category.into(),
            description: String::new(),
            help: String::new(),
            pins: Vec::new(),
            properties: Vec::new(),
            cost: 1,
            adaptive: false,
            pure: false,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    pub fn pins(mut self, pins: Vec<PinDefinition>) -> Self {
        self.pins = pins;
        self
    }

    pub fn properties(mut self, properties: Vec<PropertyDefinition>) -> Self {
        self.properties = properties;
        self
    }

    pub fn cost(mut self, cost: u32) -> Self {
        self.cost = cost;
        self
    }

    pub fn pure(mut self) -> Self {
        self.pure = true;
        self
    }
}

impl From<NodeDefinition> for ticknode_shared::NodeDefinition {
    fn from(d: NodeDefinition) -> Self {
        Self {
            type_name: d.type_name,
            title: d.title,
            category: d.category,
            description: d.description,
            help: d.help,
            pins: d.pins,
            properties: d.properties,
            cost: d.cost,
            adaptive: d.adaptive,
        }
    }
}

impl From<ticknode_shared::NodeDefinition> for NodeDefinition {
    fn from(d: ticknode_shared::NodeDefinition) -> Self {
        Self {
            type_name: d.type_name,
            title: d.title,
            category: d.category,
            description: d.description,
            help: d.help,
            pins: d.pins,
            properties: d.properties,
            cost: d.cost,
            adaptive: d.adaptive,
            pure: false,
        }
    }
}
