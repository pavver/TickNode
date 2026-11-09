use serde::{Deserialize, Serialize};
use crate::{NodeSize, DetachedConnection, WireRoute, NodeId, PinId, Position, DataValue, ConnectionModel, GraphModel, SettingUpdate, NodeDefinition, NodeInstance, TypeDefinition, PluginInfo, Translations, PropertyOptionSet, PropertyOption, UiMessage};

// --- Server <-> editor protocol (postcard over WebSocket) ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    AddNode { type_name: String, position: Position },
    RemoveNode(NodeId),
    MoveNode { id: NodeId, position: Position },
    SetProperty { id: NodeId, name: String, value: DataValue },
    ConnectExec(ConnectionModel),
    DisconnectExec(ConnectionModel),
    ConnectData(ConnectionModel),
    DisconnectData { dst_node: NodeId, dst_pin: PinId },
    TriggerNode { id: NodeId, pin: PinId },
    /// Value of an unconnected data input; `None` clears it (the node falls back to its default).
    SetInputValue { id: NodeId, pin: PinId, value: Option<DataValue> },
    /// Paste a graph fragment (copy/duplicate). Nodes receive new IDs;
    /// positions are taken from the fragment as-is.
    Paste(GraphModel),
    /// Enable or disable a plugin on the fly (without restarting the server).
    SetPluginEnabled { name: String, enabled: bool },
    /// Save plugin settings. Keys not present in the list remain unchanged.
    ConfigurePlugin { name: String, values: Vec<(String, SettingUpdate)> },
    /// Replace a named struct pin with its fields, or restore the original pin.
    SetPinSplit { node: NodeId, pin: PinId, split: bool },
    Undo,
    Redo,
    /// Applies graph edits as one shared history entry. History, runtime and plugin commands are rejected.
    Batch(Vec<ClientMessage>),
    /// Replaces all stored settings of an existing node of the same type.
    PasteSettings { id: NodeId, type_name: String, properties: Vec<(String, DataValue)> },
    /// Creates and connects a node as one atomic graph edit.
    AddNodeConnected { type_name: String, position: Position, from: ConnectionEnd, pin: PinId },
    /// Replaces layout points of an existing connection; an empty list clears the route.
    SetWireRoute(WireRoute),
    ConfirmPins(NodeId),
    RemoveDetached(DetachedConnection),
    /// Sets layout dimensions; None restores automatic sizing.
    ResizeNode { id: NodeId, size: Option<NodeSize> },
}

/// An existing endpoint from which a connection is dragged.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionEnd {
    pub node: NodeId,
    pub pin: PinId,
}

/// Shared server history availability for every editor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
}

/// What a node received and emitted during its last execution (for the editor inspector).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeTrace {
    pub node_id: NodeId,
    pub tick: u64,
    /// Connected data inputs; `None` means the source had no value yet.
    pub inputs: Vec<(PinId, Option<DataValue>)>,
    /// Known values of output data pins after execution.
    pub outputs: Vec<(PinId, DataValue)>,
    /// Exec outputs that fired.
    pub fired: Vec<PinId>,
}

/// Progress of the first started host delay for a node; count includes every active delay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeTimer {
    pub node: NodeId,
    pub total_ms: u64,
    pub remaining_ms: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// Full state snapshot — sent immediately after connecting.
    Init {
        definitions: Vec<NodeDefinition>,
        nodes: Vec<NodeInstance>,
        exec_connections: Vec<ConnectionModel>,
        data_connections: Vec<ConnectionModel>,
        /// Latest node executions and how many milliseconds ago they occurred.
        traces: Vec<(NodeTrace, u64)>,
        /// Custom data types declared by plugins.
        types: Vec<TypeDefinition>,
        /// All discovered plugins with their state and settings (secrets only indicate whether set).
        plugins: Vec<PluginInfo>,
        /// Translation resources of the engine core and every enabled plugin, merged per locale.
        translations: Translations,
        /// Runtime options of dynamic properties published by enabled plugins.
        #[serde(default)]
        property_options: Vec<PropertyOptionSet>,
        history: HistoryState,
        #[serde(default)]
        wire_routes: Vec<WireRoute>,
        #[serde(default)]
        detached_connections: Vec<DetachedConnection>,
        #[serde(default)]
        timers: Vec<NodeTimer>,
    },
    NodeAdded(NodeInstance),
    NodeRemoved(NodeId),
    NodeMoved { id: NodeId, position: Position },
    /// Properties changed (the definition may have changed as well).
    NodeUpdated(NodeInstance),
    ExecConnected(ConnectionModel),
    ExecDisconnected(ConnectionModel),
    DataConnected(ConnectionModel),
    DataDisconnected { dst_node: NodeId, dst_pin: PinId },
    /// Nodes executed on the latest tick, with their inputs and outputs.
    Executed(Vec<NodeTrace>),
    /// Sent only to the initiator of `AddNode` / `AddNodeConnected` / `Paste`: IDs of created nodes (to select them).
    /// Arrives after the corresponding `NodeAdded` messages.
    Created(Vec<NodeId>),
    Log { node_id: NodeId, message: String },
    /// Plugin state or settings changed.
    PluginUpdated(PluginInfo),
    /// The set of node and data types changed (plugin enabled or disabled): new definitions for the palette.
    /// Affected nodes arrive separately as `NodeUpdated`.
    Catalog { definitions: Vec<NodeDefinition>, types: Vec<TypeDefinition>, translations: Translations },
    /// A plugin replaced the options of a dynamic property source (an empty list removes it).
    PropertyOptions { plugin: String, source: String, options: Vec<PropertyOption> },
    /// Error for the client that caused it; translatable.
    Error(UiMessage),
    History(HistoryState),
    WireRouted(WireRoute),
    Detached(DetachedConnection),
    DetachedRemoved(DetachedConnection),
    /// Full current list, emitted only when the first delay or count changes.
    Timers(Vec<NodeTimer>),
    NodeResized { id: NodeId, size: Option<NodeSize> },
}
