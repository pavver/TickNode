//! The engine lives on a dedicated thread (actor): ticks and editor commands are processed sequentially,
//! so `TickEngine` needs no locks. The thread does not belong to tokio: plugin calls
//! (synchronous wasmtime-wasi) cannot be made inside an async context. Network tasks
//! are spawned by the engine onto the server runtime via `HostState::runtime`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ticknode_core::{EngineError, RemovedConnections, TickEngine};
use ticknode_shared::{
    ClientMessage, Frame, FrameId, NodeId, NodeInstance, NodeTrace, NodeSize,
    Position, ServerMessage, SheetId, SheetInfo, SheetSummary, UiMessage,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{error, info};

use crate::plugins::Plugins;
use crate::storage::{LoadedSheets, Storage};

const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);
/// A larger fragment is almost certainly a bug or abuse.
const MAX_PASTE_NODES: usize = 1000;

pub enum Command {
    Client {
        msg: ClientMessage,
        sheet: Option<SheetId>,
        reply: mpsc::UnboundedSender<ServerMessage>,
    },
    Snapshot {
        sheet: Option<SheetId>,
        reply: oneshot::Sender<ServerMessage>,
    },
    OpenSheet {
        sheet: Option<SheetId>,
        reply: oneshot::Sender<Result<ServerMessage, UiMessage>>,
    },
    Shutdown(oneshot::Sender<()>),
}

/// Handle to the engine actor for HTTP/WebSocket handlers.
#[derive(Clone)]
pub struct EngineHandle {
    commands: std_mpsc::Sender<Command>,
    events: broadcast::Sender<(Option<SheetId>, Bytes)>,
}

impl EngineHandle {
    /// Events broadcast to all editors, tagged by sheet (or None for global), pre-encoded with postcard.
    pub fn subscribe(&self) -> broadcast::Receiver<(Option<SheetId>, Bytes)> {
        self.events.subscribe()
    }

    pub fn send(&self, msg: ClientMessage, sheet: Option<SheetId>, reply: mpsc::UnboundedSender<ServerMessage>) {
        let _ = self.commands.send(Command::Client { msg, sheet, reply });
    }

    pub async fn snapshot(&self, sheet: Option<SheetId>) -> Option<ServerMessage> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Snapshot { sheet, reply: tx }).ok()?;
        rx.await.ok()
    }

    pub async fn open_sheet(&self, sheet: Option<SheetId>) -> Result<ServerMessage, UiMessage> {
        let (tx, rx) = oneshot::channel();
        if self.commands.send(Command::OpenSheet { sheet, reply: tx }).is_err() {
            return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
        }
        rx.await.unwrap_or_else(|_| Err(UiMessage::new("error-sheet-not-found", "Sheet not found")))
    }

    /// Stops the engine and waits for the final graph save to complete.
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        if self.commands.send(Command::Shutdown(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}

pub fn encode(msg: &ServerMessage) -> Bytes {
    Bytes::from(postcard::to_allocvec(msg).expect("ServerMessage always serializes"))
}

pub fn spawn(
    mut engine: TickEngine,
    loaded: LoadedSheets,
    storage: Storage,
    plugins: Plugins,
    tick: Duration,
) -> std::io::Result<EngineHandle> {
    let (commands, rx) = std_mpsc::channel();
    let (events, _) = broadcast::channel(1024);
    engine.host_state.runtime = Some(tokio::runtime::Handle::current());
    engine.host_state.tick_duration = tick;
    let actor = Actor {
        engine,
        sheets: loaded.sheets,
        node_sheet: loaded.node_sheet,
        frame_sheet: loaded.frame_sheet,
        histories: HashMap::new(),
        positions: loaded.positions,
        sizes: loaded.sizes,
        routes: loaded.routes,
        frames: loaded.frames,
        storage,
        plugins,
        events: events.clone(),
        dirty_since: None,
        dirty_sheets: HashSet::new(),
        dirty_sheets_meta: false,
        traces: HashMap::new(),
        telemetry: telemetry::Telemetry::new(Instant::now()),
        last_sheets_broadcast: Instant::now(),
        last_sheet_summaries: Vec::new(),
    };
    std::thread::Builder::new()
        .name("ticknode-engine".into())
        .spawn(move || actor.run(rx, tick))?;
    Ok(EngineHandle { commands, events })
}

pub(super) struct Actor {
    pub(super) engine: TickEngine,
    pub(super) sheets: Vec<SheetInfo>,
    pub(super) node_sheet: HashMap<NodeId, SheetId>,
    pub(super) frame_sheet: HashMap<FrameId, SheetId>,
    pub(super) histories: HashMap<SheetId, history::History>,
    pub(super) positions: HashMap<NodeId, Position>,
    pub(super) sizes: HashMap<NodeId, NodeSize>,
    pub(super) routes: routes::Routes,
    pub(super) frames: BTreeMap<FrameId, Frame>,
    pub(super) storage: Storage,
    pub(super) plugins: Plugins,
    pub(super) events: broadcast::Sender<(Option<SheetId>, Bytes)>,
    /// When the graph was first modified since the last save.
    pub(super) dirty_since: Option<Instant>,
    pub(super) dirty_sheets: HashSet<SheetId>,
    pub(super) dirty_sheets_meta: bool,
    /// Latest execution of each node (in-memory only: runtime state, not part of the graph).
    pub(super) traces: HashMap<NodeId, (NodeTrace, Instant)>,
    /// Traces and logs waiting for the next telemetry interval.
    pub(super) telemetry: telemetry::Telemetry,
    pub(super) last_sheets_broadcast: Instant,
    pub(super) last_sheet_summaries: Vec<SheetSummary>,
}

impl Actor {
    fn run(mut self, rx: std_mpsc::Receiver<Command>, tick: Duration) {
        let mut next_tick = Instant::now() + tick;
        loop {
            let now = Instant::now();
            if now >= next_tick {
                self.tick();
                next_tick += tick;
                // Do not catch up on missed ticks (if the engine was busy longer than a tick).
                if next_tick <= now {
                    next_tick = now + tick;
                }
            }
            if self.dirty_since.is_some_and(|t| now >= t + SAVE_DEBOUNCE) {
                self.save();
            }

            let deadline = self.dirty_since.map_or(next_tick, |t| next_tick.min(t + SAVE_DEBOUNCE));
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(command) => { if self.handle_command(command) { return; } }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    self.save();
                    return;
                }
            }
        }
    }

    fn handle_command(&mut self, command: Command) -> bool {
        match command {
            Command::Client { msg, sheet, reply } => match self.apply(msg, sheet, &reply) {
                Ok((events, created)) => {
                    for (tag, event) in events {
                        self.broadcast(tag, &event);
                    }
                    self.broadcast_option_changes();
                    if let Some(ids) = created {
                        let _ = reply.send(ServerMessage::Created(ids));
                    }
                }
                Err(e) => { let _ = reply.send(ServerMessage::Error(e)); }
            },
            Command::Snapshot { sheet, reply } => { let _ = reply.send(self.snapshot(sheet)); }
            Command::OpenSheet { sheet, reply } => {
                if let Some(id) = sheet {
                    if !self.sheets.iter().any(|s| s.id == id) {
                        let _ = reply.send(Err(UiMessage::new("error-sheet-not-found", "Sheet not found")));
                        return false;
                    }
                }
                let _ = reply.send(Ok(self.snapshot(sheet)));
            }
            Command::Shutdown(done) => {
                self.save();
                info!("Engine stopped");
                let _ = done.send(());
                return true;
            }
        }
        false
    }

    fn broadcast_option_changes(&mut self) {
        for set in self.engine.take_property_option_changes() {
            self.broadcast(None, &ServerMessage::PropertyOptions { plugin: set.plugin, source: set.source, options: set.options });
        }
    }

    fn tick(&mut self) {
        self.engine.tick();
        self.broadcast_option_changes();
        let now = Instant::now();
        for trace in &self.engine.executed {
            self.traces.insert(trace.node_id, (trace.clone(), now));
        }
        if self.events.receiver_count() == 0 {
            self.engine.host_state.logs.clear();
            self.telemetry.discard();
            self.engine.take_node_error_changes();
            return;
        }
        self.telemetry.add_traces(&self.engine.executed);
        self.telemetry.add_logs(std::mem::take(&mut self.engine.host_state.logs));
        for (tag, message) in self.telemetry.flush(now, &mut self.engine, &self.node_sheet) {
            self.broadcast(tag, &message);
        }
        self.check_sheets_broadcast(now);
    }

    pub(crate) fn broadcast(&self, sheet: Option<SheetId>, msg: &ServerMessage) {
        let _ = self.events.send((sheet, encode(msg)));
    }

    fn save(&mut self) {
        if self.dirty_since.is_none() {
            return;
        }
        let mut ok = true;
        if self.dirty_sheets_meta || !self.storage.sheets_json_path().exists() {
            if let Err(e) = self.storage.save_sheets_meta(&self.sheets) {
                error!("Failed to save sheets.json: {:#}", e);
                ok = false;
            } else {
                self.dirty_sheets_meta = false;
            }
        }
        let mut dirty_list: Vec<SheetId> = self.dirty_sheets.iter().copied().collect();
        if dirty_list.is_empty() {
            dirty_list = self.sheets.iter().map(|s| s.id).collect();
        }
        for id in dirty_list {
            let graph = self.sheet_graph(id);
            if let Err(e) = self.storage.save_sheet(id, &graph) {
                error!("Failed to save sheet {}: {:#}", id, e);
                ok = false;
            } else {
                self.dirty_sheets.remove(&id);
            }
        }
        if ok {
            self.dirty_since = None;
        } else {
            self.dirty_since = Some(Instant::now());
        }
    }

    pub(crate) fn instance(&self, id: NodeId) -> Option<NodeInstance> {
        let mut model = self.engine.node_model(id)?;
        model.position = self.positions.get(&id).copied().unwrap_or_default();
        model.size = self.sizes.get(&id).copied();
        Some(NodeInstance {
            model,
            definition: self.engine.node_definition(id)?,
            unavailable: self.engine.node_unavailable(id),
            frozen: self.engine.frozen_changes(id).to_vec(),
        })
    }

    pub(crate) fn snapshot(&self, open_sheet: Option<SheetId>) -> ServerMessage {
        let active_sheet = open_sheet.or_else(|| self.sheets.first().map(|s| s.id));
        let summaries = self.sheet_summaries();
        let (graph, traces, history, timers, node_errors) = match active_sheet {
            Some(sheet_id) => {
                let g = self.sheet_graph(sheet_id);
                let t = self.traces.values()
                    .filter(|(trace, _)| self.node_sheet.get(&trace.node_id) == Some(&sheet_id))
                    .map(|(trace, at)| (trace.clone(), at.elapsed().as_millis() as u64))
                    .collect();
                let h = self.histories.get(&sheet_id).map_or_default(|h| h.state());
                let timers = self.engine.delay_timers().into_iter()
                    .filter(|t| self.node_sheet.get(&t.node) == Some(&sheet_id))
                    .collect();
                let errors = self.engine.node_errors().into_iter()
                    .filter(|e| self.node_sheet.get(&e.node) == Some(&sheet_id))
                    .collect();
                (g, t, h, timers, errors)
            }
            None => (
                ticknode_shared::GraphModel::default(),
                Vec::new(),
                ticknode_shared::HistoryState::default(),
                Vec::new(),
                Vec::new(),
            ),
        };

        ServerMessage::Init {
            definitions: self.engine.registry.get_definitions(),
            types: self.engine.types.definitions(),
            plugins: self.plugins.infos(&self.engine),
            translations: self.engine.translations(),
            property_options: self.engine.property_options(),
            history,
            wire_routes: graph.wire_routes,
            detached_connections: graph.detached_connections,
            timers,
            node_errors,
            nodes: graph.nodes.iter().filter_map(|n| self.instance(n.id)).collect(),
            exec_connections: graph.exec_connections,
            data_connections: graph.data_connections,
            traces,
            frames: graph.frames,
            sheets: summaries,
            open_sheet: active_sheet,
        }
    }

    fn catalog(&self) -> ServerMessage {
        ServerMessage::Catalog {
            definitions: self.engine.registry.get_definitions(),
            types: self.engine.types.definitions(),
            translations: self.engine.translations(),
        }
    }
}

pub(crate) mod routes;
pub(crate) mod sheets;
mod commands;
mod history;
mod telemetry;
mod settings;
mod resize;
mod frames;

#[cfg(test)]
mod tests;
