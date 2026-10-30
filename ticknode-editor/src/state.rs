//! Editor state. The server is the single source of truth: graph mutations are applied only via `ServerMessage`.

use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use ticknode_shared::*;
use web_sys::WebSocket;

use crate::geometry::Camera;
use crate::i18n::{self, Locale};

const MAX_LOG_LINES: usize = 500;
/// Duration (ms) a node stays highlighted after execution.
pub const EXEC_HIGHLIGHT_MS: f64 = 250.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnStatus {
    Connecting,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Node,
    Error,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub id: u64,
    pub time: String,
    pub kind: LogKind,
    pub text: String,
}

/// Selection: one or more nodes, or a single wire.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// Never empty (an empty selection is represented as `None`).
    Nodes(BTreeSet<NodeId>),
    Exec(ConnectionModel),
    Data(ConnectionModel),
}

/// Dragging a node from the palette onto the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteDrag {
    pub type_name: String,
    pub title: String,
    pub color: &'static str,
    pub start: (f64, f64),
    /// Current cursor position (client coordinates).
    pub at: (f64, f64),
    /// Cursor moved past the threshold — this is now a drag rather than a click.
    pub active: bool,
}

/// Node "info" tooltip: text and bounding rect of the button (client coordinates) to anchor next to.
#[derive(Debug, Clone, PartialEq)]
pub struct Tooltip {
    pub title: String,
    pub text: String,
    pub anchor: (f64, f64, f64, f64),
}

/// Paste awaiting placement: the fragment follows the cursor; LMB places it, RMB/Esc cancels.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPaste {
    /// Node positions relative to the top-left corner of the fragment.
    pub graph: GraphModel,
    /// Current target position for the top-left corner (world coordinates, grid-snapped).
    pub at: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

/// Client-side node: fine-grained signals so dragging doesn't re-render the rest of the graph.
#[derive(Clone, Copy)]
pub struct ClientNode {
    pub instance: RwSignal<NodeInstance>,
    pub pos: RwSignal<Position>,
    /// Local timestamp (ms) of the last execution.
    pub last_exec: RwSignal<f64>,
    /// Inputs and outputs from the last execution.
    pub trace: RwSignal<Option<NodeTrace>>,
}

/// Node signals are disposed after the UI finishes re-rendering: otherwise effects still
/// referencing the node (inspector, inline fields) would run against disposed signals.
fn dispose_later(nodes: impl IntoIterator<Item = ClientNode> + 'static) {
    set_timeout(move || nodes.into_iter().for_each(ClientNode::dispose), std::time::Duration::ZERO);
}

impl ClientNode {
    fn new(instance: NodeInstance) -> Self {
        Self {
            pos: RwSignal::new(instance.model.position),
            instance: RwSignal::new(instance),
            last_exec: RwSignal::new(f64::NEG_INFINITY),
            trace: RwSignal::new(None),
        }
    }

    fn set_trace(&self, trace: NodeTrace, at: f64) {
        self.last_exec.set(at);
        self.trace.set(Some(trace));
    }

    fn dispose(self) {
        self.instance.dispose();
        self.pos.dispose();
        self.last_exec.dispose();
        self.trace.dispose();
    }
}

#[derive(Clone, Copy)]
pub struct Editor {
    pub status: RwSignal<ConnStatus>,
    pub definitions: RwSignal<Vec<NodeDefinition>>,
    /// Custom data types declared by plugins (for colors, labels, and dropdowns).
    pub types: RwSignal<TypeRegistry>,
    pub nodes: RwSignal<BTreeMap<NodeId, ClientNode>>,
    pub exec: RwSignal<Vec<ConnectionModel>>,
    pub data: RwSignal<Vec<ConnectionModel>>,
    pub selection: RwSignal<Option<Selection>>,
    pub logs: RwSignal<Vec<LogLine>>,
    /// Server error counter: property fields in Pending transition to Error when this increments.
    pub error_seq: RwSignal<u64>,
    pub view: RwSignal<Camera>,
    /// Current timestamp (ms), updated by a timer — used to fade out execution highlights.
    pub now: RwSignal<f64>,
    pub theme: RwSignal<Theme>,
    pub locale: RwSignal<Locale>,
    pub show_palette: RwSignal<bool>,
    pub show_console: RwSignal<bool>,
    /// Plugins panel (replaces the inspector slot).
    pub show_plugins: RwSignal<bool>,
    /// Plugins with their state and settings (secrets only indicate whether they are set).
    pub plugins: RwSignal<Vec<PluginInfo>>,
    /// Narrow screen: panels become slide-over drawers above the canvas.
    pub narrow: RwSignal<bool>,
    pub socket: StoredValue<Option<WebSocket>, LocalStorage>,
    /// Last cursor position over the canvas (world coordinates) — target for paste.
    pub pointer_world: StoredValue<Option<(f64, f64)>>,
    pub palette_drag: RwSignal<Option<PaletteDrag>>,
    pub pending_paste: RwSignal<Option<PendingPaste>>,
    pub tooltip: RwSignal<Option<Tooltip>>,
    /// Timestamp of the last middle-button press/release (`INFINITY` while held). On Linux,
    /// middle-click pastes the primary selection via a `paste` event — we ignore such pastes.
    pub middle_button_at: StoredValue<f64>,
    /// Fit camera to the graph only after the first Init, not on every reconnect.
    fitted: StoredValue<bool>,
    next_log_id: StoredValue<u64>,
}

pub fn use_editor() -> Editor {
    expect_context::<Editor>()
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn clock_time() -> String {
    let d = js_sys::Date::new_0();
    format!("{:02}:{:02}:{:02}", d.get_hours(), d.get_minutes(), d.get_seconds())
}

impl Editor {
    pub fn new(theme: Theme, locale: RwSignal<Locale>, wide_screen: bool) -> Self {
        Self {
            status: RwSignal::new(ConnStatus::Connecting),
            definitions: RwSignal::new(Vec::new()),
            types: RwSignal::new(TypeRegistry::new()),
            nodes: RwSignal::new(BTreeMap::new()),
            exec: RwSignal::new(Vec::new()),
            data: RwSignal::new(Vec::new()),
            selection: RwSignal::new(None),
            logs: RwSignal::new(Vec::new()),
            error_seq: RwSignal::new(0),
            view: RwSignal::new(Camera::default()),
            now: RwSignal::new(now_ms()),
            theme: RwSignal::new(theme),
            locale,
            show_palette: RwSignal::new(wide_screen),
            show_console: RwSignal::new(wide_screen),
            show_plugins: RwSignal::new(false),
            plugins: RwSignal::new(Vec::new()),
            narrow: RwSignal::new(!wide_screen),
            socket: StoredValue::new_local(None),
            pointer_world: StoredValue::new(None),
            palette_drag: RwSignal::new(None),
            pending_paste: RwSignal::new(None),
            tooltip: RwSignal::new(None),
            middle_button_at: StoredValue::new(f64::NEG_INFINITY),
            fitted: StoredValue::new(false),
            next_log_id: StoredValue::new(0),
        }
    }

    pub fn log(&self, kind: LogKind, text: impl Into<String>) {
        let id = self.next_log_id.get_value();
        self.next_log_id.set_value(id + 1);
        let line = LogLine { id, time: clock_time(), kind, text: text.into() };
        self.logs.update(|logs| {
            logs.push(line);
            if logs.len() > MAX_LOG_LINES {
                let excess = logs.len() - MAX_LOG_LINES;
                logs.drain(..excess);
            }
        });
    }

    pub fn send(&self, msg: ClientMessage) {
        let sent = self.socket.with_value(|socket| match (socket, postcard::to_allocvec(&msg)) {
            (Some(ws), Ok(bytes)) => ws.send_with_u8_array(&bytes).is_ok(),
            _ => false,
        });
        if !sent {
            self.log(LogKind::Error, "[ERROR] Disconnected from server - change was not sent");
        }
    }

    /// Whether the node is selected (tracked).
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection.with(|s| matches!(s, Some(Selection::Nodes(set)) if set.contains(&id)))
    }

    pub fn selected_nodes(&self) -> Vec<NodeId> {
        self.selection.with_untracked(|s| match s {
            Some(Selection::Nodes(set)) => set.iter().copied().collect(),
            _ => Vec::new(),
        })
    }

    pub fn select_nodes(&self, ids: impl IntoIterator<Item = NodeId>) {
        let set: BTreeSet<NodeId> = ids.into_iter().collect();
        self.selection.set((!set.is_empty()).then_some(Selection::Nodes(set)));
    }

    /// Removes deleted nodes and wires referencing them from the current selection.
    fn prune_selection(&self) {
        let exists = |id: &NodeId| self.node(*id).is_some();
        let next = match self.selection.get_untracked() {
            Some(Selection::Nodes(set)) => {
                let kept: BTreeSet<_> = set.iter().copied().filter(exists).collect();
                if kept.len() == set.len() { return }
                (!kept.is_empty()).then_some(Selection::Nodes(kept))
            }
            Some(Selection::Exec(c)) if !self.exec.with_untracked(|e| e.contains(&c)) => None,
            Some(Selection::Data(c)) if !self.data.with_untracked(|d| d.contains(&c)) => None,
            _ => return,
        };
        self.selection.set(next);
    }

    /// Node lookup without subscribing to the node map (event handlers, applying messages).
    pub fn node(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with_untracked(|nodes| nodes.get(&id).copied())
    }

    /// Tracked node lookup: for computations that should update when the node appears or disappears.
    pub fn node_tracked(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with(|nodes| nodes.get(&id).copied())
    }

    pub fn node_title(&self, id: NodeId) -> String {
        self.node(id)
            .map(|n| n.instance.with_untracked(|i| i18n::node_title(&i.definition)))
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn is_input_connected(&self, node: NodeId, pin: PinId) -> bool {
        self.data.with(|d| d.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
            || self.exec.with(|e| e.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
    }

    /// Data type of the output pin connected to a data input (tracked).
    pub fn source_type(&self, node: NodeId, pin: PinId) -> Option<DataType> {
        let src = self.data.with(|d| d.iter().find(|c| c.dst_node == node && c.dst_pin == pin).map(|c| (c.src_node, c.src_pin)))?;
        self.node_tracked(src.0)?.instance.with(|i| {
            i.definition.pins.iter()
                .find(|p| p.id == src.1 && p.direction == PinDirection::Output)
                .and_then(|p| match &p.pin_type {
                    PinType::Data(t) => Some(t.clone()),
                    PinType::Exec => None,
                })
        })
    }

    /// Required data input that is neither connected nor has an inline value on the node (tracked).
    pub fn required_missing(&self, node: NodeId, pin: &PinDefinition) -> bool {
        if !pin.required || pin.direction != PinDirection::Input || pin.pin_type == PinType::Exec {
            return false;
        }
        let name = input_value_property(pin.id);
        !self.is_input_connected(node, pin.id)
            && !self.node_tracked(node).is_some_and(|n| n.instance.with(|i| i.model.properties.iter().any(|(p, _)| *p == name)))
    }

    /// Human-readable type name (`title` for custom types), tracked against the type registry.
    pub fn type_title(&self, t: &DataType) -> String {
        self.types.with(|types| i18n::data_type_title(types, t))
    }

    pub fn is_output_connected(&self, node: NodeId, pin: PinId) -> bool {
        self.data.with(|d| d.iter().any(|c| c.src_node == node && c.src_pin == pin))
            || self.exec.with(|e| e.iter().any(|c| c.src_node == node && c.src_pin == pin))
    }

    fn upsert_node(&self, instance: NodeInstance) -> NodeId {
        let id = instance.model.id;
        match self.node(id) {
            Some(existing) => {
                existing.pos.set(instance.model.position);
                existing.instance.set(instance);
            }
            None => self.nodes.update(|nodes| {
                nodes.insert(id, ClientNode::new(instance));
            }),
        }
        id
    }

    fn set_types(&self, types: Vec<TypeDefinition>) {
        let registry = TypeRegistry::from_definitions(types);
        i18n::set_known_types(&registry);
        self.types.set(registry);
    }

    pub fn apply(&self, msg: ServerMessage) {
        match msg {
            ServerMessage::Init { definitions, nodes, exec_connections, data_connections, traces, types, plugins, translations } => {
                // Update existing nodes in place (keeping open inspector and inline fields alive
                // across reconnects) and remove any that disappeared
                let fresh: BTreeSet<NodeId> = nodes.iter().map(|i| i.model.id).collect();
                let gone: Vec<ClientNode> = self.nodes.try_update(|map| {
                    let ids: Vec<NodeId> = map.keys().filter(|id| !fresh.contains(id)).copied().collect();
                    ids.iter().filter_map(|id| map.remove(id)).collect()
                }).unwrap_or_default();
                let count = nodes.len();
                i18n::set_runtime_translations(&translations);
                self.set_types(types);
                self.definitions.set(definitions);
                self.plugins.set(plugins);
                for instance in nodes {
                    self.upsert_node(instance);
                }
                dispose_later(gone);
                self.exec.set(exec_connections);
                self.data.set(data_connections);
                let now = now_ms();
                for (trace, ago_ms) in traces {
                    if let Some(node) = self.node(trace.node_id) {
                        node.set_trace(trace, now - ago_ms as f64);
                    }
                }
                self.prune_selection();
                self.log(LogKind::System, format!("[UI] Graph received: {count} {}", if count == 1 { "node" } else { "nodes" }));
                if !self.fitted.get_value() {
                    self.fitted.set_value(true);
                    let ed = *self;
                    // Wait until nodes are mounted in the DOM and the canvas has dimensions
                    request_animation_frame(move || crate::canvas::fit_view(ed));
                }
            }
            ServerMessage::NodeAdded(instance) => {
                self.upsert_node(instance);
            }
            ServerMessage::NodeUpdated(instance) => {
                self.upsert_node(instance);
            }
            ServerMessage::NodeRemoved(id) => {
                let removed = self.nodes.try_update(|nodes| nodes.remove(&id)).flatten();
                self.exec.update(|e| e.retain(|c| c.src_node != id && c.dst_node != id));
                self.data.update(|d| d.retain(|c| c.src_node != id && c.dst_node != id));
                self.prune_selection();
                dispose_later(removed);
            }
            ServerMessage::NodeMoved { id, position } => {
                if let Some(node) = self.node(id) {
                    if node.pos.get_untracked() != position {
                        node.pos.set(position);
                    }
                }
            }
            ServerMessage::ExecConnected(c) => self.exec.update(|e| {
                if !e.contains(&c) {
                    e.push(c);
                }
            }),
            ServerMessage::ExecDisconnected(c) => {
                self.exec.update(|e| e.retain(|x| *x != c));
                self.prune_selection();
            }
            ServerMessage::DataConnected(c) => self.data.update(|d| {
                // A data input has a single source: a new connection replaces the old one
                d.retain(|x| x.dst_node != c.dst_node || x.dst_pin != c.dst_pin);
                d.push(c);
            }),
            ServerMessage::DataDisconnected { dst_node, dst_pin } => {
                self.data.update(|d| d.retain(|x| x.dst_node != dst_node || x.dst_pin != dst_pin));
                self.prune_selection();
            }
            ServerMessage::Executed(traces) => {
                let now = now_ms();
                for trace in traces {
                    if let Some(node) = self.node(trace.node_id) {
                        node.set_trace(trace, now);
                    }
                }
            }
            ServerMessage::PluginUpdated(info) => self.plugins.update(|plugins| {
                match plugins.iter_mut().find(|p| p.name == info.name) {
                    Some(existing) => *existing = info,
                    None => {
                        plugins.push(info);
                        plugins.sort_by(|a, b| a.name.cmp(&b.name));
                    }
                }
            }),
            // Plugin enabled or disabled: refresh palette and type colors from the updated catalog
            ServerMessage::Catalog { definitions, types, translations } => {
                i18n::set_runtime_translations(&translations);
                self.set_types(types);
                self.definitions.set(definitions);
            }
            // Nodes created in response to our AddNode / Paste (sent by the server after their NodeAdded messages)
            ServerMessage::Created(ids) => self.select_nodes(ids),
            ServerMessage::Log { node_id, message } => {
                // Logs stay English: the node is named by its definition title, not a translation
                let title = self.node(node_id)
                    .map(|n| n.instance.with_untracked(|i| i.definition.title.clone()))
                    .unwrap_or_else(|| "?".to_string());
                self.log(LogKind::Node, format!("[{title}] {message}"));
            }
            ServerMessage::Error(message) => {
                self.error_seq.update(|n| *n += 1);
                // The answer to the user's action: translated, unlike technical log lines
                self.log(LogKind::Error, format!("[ERROR] {}", i18n::ui_text(&message)));
            }
        }
    }
}
