use serde::{Deserialize, Serialize};
use crate::{Uuid, PinId, DataValue, NodeDefinition, UiMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConnectionModel {
    pub src_node: NodeId,
    pub src_pin: PinId,
    pub dst_node: NodeId,
    pub dst_pin: PinId,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Persisted state of a single node: type + properties from which it can be recreated via the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeModel {
    pub id: NodeId,
    pub type_name: String,
    pub properties: Vec<(String, DataValue)>,
    pub position: Position,
}

/// Editor routing of a wire through intermediate points (layout only, ignored by execution).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireRoute {
    pub exec: bool,
    pub connection: ConnectionModel,
    pub points: Vec<Position>,
}

pub const MAX_WIRE_POINTS: usize = 32;

/// Complete graph (or a fragment for copying) in a format independent of node implementations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GraphModel {
    pub nodes: Vec<NodeModel>,
    pub exec_connections: Vec<ConnectionModel>,
    pub data_connections: Vec<ConnectionModel>,
    #[serde(default)]
    pub wire_routes: Vec<WireRoute>,
}

/// A node paired with its current definition (pins may depend on properties).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInstance {
    pub model: NodeModel,
    pub definition: NodeDefinition,
    /// Stub node: the plugin for its type is unavailable (disabled or not loaded). The message is an explanation
    /// for the editor. Properties and pins are preserved, but the node cannot execute or be edited.
    #[serde(default)]
    pub unavailable: Option<UiMessage>,
}
