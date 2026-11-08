//! Editor state. The server is the single source of truth: graph mutations are applied only via `ServerMessage`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use leptos::prelude::*;
use ticknode_shared::*;
use web_sys::WebSocket;

use crate::geometry::Camera;
use crate::i18n::{self, Locale};

mod graph;
mod messages;

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

/// The open searchable picker of a dynamic property (`PropertyKind::Dynamic`): which property it edits
/// and the field's bounding rect (client coordinates) to anchor the popup to.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerState {
    pub node: NodeId,
    pub property: String,
    /// Where the options come from: the node's plugin and the source name.
    pub plugin: String,
    pub source: String,
    pub anchor: (f64, f64, f64, f64),
}

/// Options published by plugins for dynamic properties, by `(plugin, source)`.
pub type PropertyOptions = HashMap<(String, String), Arc<Vec<PropertyOption>>>;

/// Paste awaiting placement: the fragment follows the cursor; LMB places it, RMB/Esc cancels.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPaste {
    /// Node positions relative to the top-left corner of the fragment.
    pub graph: GraphModel,
    /// Current target position for the top-left corner (world coordinates, grid-snapped).
    pub at: (f64, f64),
}

/// Local settings clipboard, including inline input values and split pins.
#[derive(Debug, Clone, PartialEq)]
pub struct CopiedSettings {
    pub type_name: String,
    pub title: String,
    pub properties: Vec<(String, DataValue)>,
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
    pub history: RwSignal<HistoryState>,
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
    pub copied_settings: RwSignal<Option<CopiedSettings>>,
    pub tooltip: RwSignal<Option<Tooltip>>,
    /// Options of dynamic properties; the server is the source (`Init` replaces all, `PropertyOptions` one source).
    pub property_options: RwSignal<PropertyOptions>,
    /// The open property picker popup.
    pub picker: RwSignal<Option<PickerState>>,
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
            history: RwSignal::new(HistoryState::default()),
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
            copied_settings: RwSignal::new(None),
            tooltip: RwSignal::new(None),
            property_options: RwSignal::new(HashMap::new()),
            picker: RwSignal::new(None),
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

    /// Groups a single user action into one server history entry.
    pub fn send_batch(&self, mut commands: Vec<ClientMessage>) {
        match commands.len() {
            0 => {}
            1 => self.send(commands.remove(0)),
            _ => self.send(ClientMessage::Batch(commands)),
        }
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

}
