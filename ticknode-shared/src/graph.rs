use crate::{DataValue, NodeDefinition, PinChange, PinId, PinSignature, UiMessage, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FrameId(pub Uuid);

impl FrameId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for FrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FrameColor {
    #[default]
    Gray,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: FrameId,
    pub title: String,
    pub color: FrameColor,
    pub position: Position,
    pub size: NodeSize,
}

pub const MIN_FRAME_W: f64 = 120.0;
pub const MIN_FRAME_H: f64 = 80.0;
pub const MAX_FRAME_SIZE: f64 = 8000.0;
pub const MAX_FRAME_TITLE: usize = 200;
pub const MAX_FRAMES: usize = 500;

impl Frame {
    pub fn validated(mut self) -> Option<Self> {
        if !self.position.x.is_finite()
            || !self.position.y.is_finite()
            || !self.size.width.is_finite()
            || !self.size.height.is_finite()
        {
            return None;
        }
        self.size.width = self.size.width.clamp(MIN_FRAME_W, MAX_FRAME_SIZE);
        self.size.height = self.size.height.clamp(MIN_FRAME_H, MAX_FRAME_SIZE);
        if let Some((idx, _)) = self.title.char_indices().nth(MAX_FRAME_TITLE) {
            self.title.truncate(idx);
        }
        Some(self)
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

/// User-selected node dimensions in world pixels (layout only).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NodeSize {
    pub width: f64,
    pub height: f64,
}

pub const MIN_NODE_W: f64 = 160.0;
pub const MAX_NODE_W: f64 = 800.0;
pub const MAX_NODE_H: f64 = 1000.0;

impl NodeSize {
    /// Rejects non-finite dimensions and clamps finite ones to absolute layout bounds.
    pub fn validated(self) -> Option<Self> {
        (self.width.is_finite() && self.height.is_finite()).then(|| Self {
            width: self.width.clamp(MIN_NODE_W, MAX_NODE_W),
            height: self.height.clamp(0.0, MAX_NODE_H),
        })
    }
}

/// Persisted state of a single node: type + properties from which it can be recreated via the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeModel {
    pub id: NodeId,
    pub type_name: String,
    pub properties: Vec<(String, DataValue)>,
    pub position: Position,
    #[serde(default)]
    pub pins: Option<Vec<PinSignature>>,
    #[serde(default)]
    pub size: Option<NodeSize>,
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
    #[serde(default)]
    pub detached_connections: Vec<DetachedConnection>,
    #[serde(default)]
    pub frames: Vec<Frame>,
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
    #[serde(default)]
    pub frozen: Vec<PinChange>,
}

/// The endpoint whose old pin no longer accepts the connection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DetachedEnd {
    Source,
    Target,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DetachedConnection {
    pub exec: bool,
    pub connection: ConnectionModel,
    pub end: DetachedEnd,
    pub row: u32,
}

#[cfg(test)]
mod tests;
