//! Стан редактора. Сервер — джерело істини: зміни графа застосовуються лише з `ServerMessage`.

use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use ticknode_shared::*;
use web_sys::WebSocket;

use crate::geometry::Camera;

const MAX_LOG_LINES: usize = 500;
/// Скільки мілісекунд нода підсвічується після виконання.
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

/// Виділення: кілька нод або один дріт.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// Ніколи не порожня (порожнє виділення — `None`).
    Nodes(BTreeSet<NodeId>),
    Exec(ConnectionModel),
    Data(ConnectionModel),
}

/// Перетягування ноди з палітри на полотно.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteDrag {
    pub type_name: String,
    pub title: String,
    pub color: &'static str,
    pub start: (f64, f64),
    /// Поточна позиція курсора (клієнтські координати).
    pub at: (f64, f64),
    /// Курсор зрушив далі за поріг — це вже перетягування, а не клік.
    pub active: bool,
}

/// Підказка «інфо» про ноду: текст і прямокутник кнопки (клієнтські координати), біля якої її показати.
#[derive(Debug, Clone, PartialEq)]
pub struct Tooltip {
    pub title: String,
    pub text: String,
    pub anchor: (f64, f64, f64, f64),
}

/// Вставка, що чекає на розміщення: фрагмент їде за курсором, ЛКМ ставить, ПКМ/Esc скасовує.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPaste {
    /// Позиції нод відносно лівого верхнього кута фрагмента.
    pub graph: GraphModel,
    /// Куди зараз потрапить лівий верхній кут (світові координати, з прив'язкою до сітки).
    pub at: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

/// Нода на клієнті: окремі сигнали, щоб перетягування не перерисовувало решту графа.
#[derive(Clone, Copy)]
pub struct ClientNode {
    pub instance: RwSignal<NodeInstance>,
    pub pos: RwSignal<Position>,
    /// Локальний час (мс) останнього виконання.
    pub last_exec: RwSignal<f64>,
    /// Входи й виходи останнього виконання.
    pub trace: RwSignal<Option<NodeTrace>>,
}

/// Сигнали нод звільняються після того, як інтерфейс перемалювався: інакше ефекти, які ще
/// тримають ноду (інспектор, поля на ній), спрацювали б на знищених сигналах.
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
    /// Власні типи даних, оголошені плагінами (для кольорів, підписів і випадних списків).
    pub types: RwSignal<TypeRegistry>,
    pub nodes: RwSignal<BTreeMap<NodeId, ClientNode>>,
    pub exec: RwSignal<Vec<ConnectionModel>>,
    pub data: RwSignal<Vec<ConnectionModel>>,
    pub selection: RwSignal<Option<Selection>>,
    pub logs: RwSignal<Vec<LogLine>>,
    /// Лічильник помилок сервера: поля властивостей у Pending переходять в Error при його зміні.
    pub error_seq: RwSignal<u64>,
    pub view: RwSignal<Camera>,
    /// Поточний час (мс), оновлюється таймером — для згасання підсвітки виконання.
    pub now: RwSignal<f64>,
    pub theme: RwSignal<Theme>,
    pub show_palette: RwSignal<bool>,
    pub show_console: RwSignal<bool>,
    /// Панель плагінів (на місці інспектора).
    pub show_plugins: RwSignal<bool>,
    /// Плагіни зі станом і налаштуваннями (секрети — лише «задано»).
    pub plugins: RwSignal<Vec<PluginInfo>>,
    /// Вузький екран: панелі — висувні шари над полотном.
    pub narrow: RwSignal<bool>,
    pub socket: StoredValue<Option<WebSocket>, LocalStorage>,
    /// Останнє положення курсора над полотном (світові координати) — куди вставляти.
    pub pointer_world: StoredValue<Option<(f64, f64)>>,
    pub palette_drag: RwSignal<Option<PaletteDrag>>,
    pub pending_paste: RwSignal<Option<PendingPaste>>,
    pub tooltip: RwSignal<Option<Tooltip>>,
    /// Коли востаннє натискали/відпускали середню кнопку (`INFINITY` — затиснута). На Linux
    /// середня кнопка вставляє «первинне виділення» подією `paste` — таку вставку ігноруємо.
    pub middle_button_at: StoredValue<f64>,
    /// Камеру вписуємо в граф лише після першого Init, а не при кожному перепідключенні.
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
    pub fn new(theme: Theme, wide_screen: bool) -> Self {
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
            self.log(LogKind::Error, "[ERROR] Немає з'єднання з сервером — зміну не надіслано");
        }
    }

    /// Чи виділена нода (з підпискою).
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

    /// Прибирає з виділення ноди, яких уже немає, і дроти, що на них посилались.
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

    /// Нода без підписки на зміни набору нод (обробники подій, застосування повідомлень).
    pub fn node(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with_untracked(|nodes| nodes.get(&id).copied())
    }

    /// Нода з підпискою: для обчислень, які мають оновитись, коли нода з'явиться чи зникне.
    pub fn node_tracked(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with(|nodes| nodes.get(&id).copied())
    }

    pub fn node_title(&self, id: NodeId) -> String {
        self.node(id)
            .map(|n| n.instance.with_untracked(|i| i.definition.title.clone()))
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn is_input_connected(&self, node: NodeId, pin: PinId) -> bool {
        self.data.with(|d| d.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
            || self.exec.with(|e| e.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
    }

    /// Тип вихідного піна, під'єднаного до входу даних (відстежується).
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

    /// Обов'язковий вхід даних, до якого нічого не під'єднано й на ноді немає значення (з підпискою).
    pub fn required_missing(&self, node: NodeId, pin: &PinDefinition) -> bool {
        if !pin.required || pin.direction != PinDirection::Input || pin.pin_type == PinType::Exec {
            return false;
        }
        let name = input_value_property(pin.id);
        !self.is_input_connected(node, pin.id)
            && !self.node_tracked(node).is_some_and(|n| n.instance.with(|i| i.model.properties.iter().any(|(p, _)| *p == name)))
    }

    /// Назва типу для людини (`title` власного типу), з підпискою на реєстр типів.
    pub fn type_title(&self, t: &DataType) -> String {
        self.types.with(|types| types.title_of(t))
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

    pub fn apply(&self, msg: ServerMessage) {
        match msg {
            ServerMessage::Init { definitions, nodes, exec_connections, data_connections, traces, types, plugins } => {
                // Наявні ноди оновлюються на місці (після перепідключення відкритий інспектор
                // і поля на нодах лишаються живими), зниклі — прибираються
                let fresh: BTreeSet<NodeId> = nodes.iter().map(|i| i.model.id).collect();
                let gone: Vec<ClientNode> = self.nodes.try_update(|map| {
                    let ids: Vec<NodeId> = map.keys().filter(|id| !fresh.contains(id)).copied().collect();
                    ids.iter().filter_map(|id| map.remove(id)).collect()
                }).unwrap_or_default();
                let count = nodes.len();
                self.types.set(TypeRegistry::from_definitions(types));
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
                self.log(LogKind::System, format!("[UI] Граф отримано: {} нод", count));
                if !self.fitted.get_value() {
                    self.fitted.set_value(true);
                    let ed = *self;
                    // Після того, як ноди потраплять у DOM і полотно матиме розмір
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
                // Вхід даних має одне джерело: нове з'єднання замінює старе
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
            // Плагін увімкнено чи вимкнено: палітра й кольори типів — з нового каталогу
            ServerMessage::Catalog { definitions, types } => {
                self.types.set(TypeRegistry::from_definitions(types));
                self.definitions.set(definitions);
            }
            // Ноди, створені на наш AddNode / Paste (сервер шле це після їхніх NodeAdded)
            ServerMessage::Created(ids) => self.select_nodes(ids),
            ServerMessage::Log { node_id, message } => {
                self.log(LogKind::Node, format!("[{}] {}", self.node_title(node_id), message));
            }
            ServerMessage::Error(text) => {
                self.error_seq.update(|n| *n += 1);
                self.log(LogKind::Error, format!("[ERROR] {}", text));
            }
        }
    }
}
