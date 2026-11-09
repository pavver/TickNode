//! The engine lives on a dedicated thread (actor): ticks and editor commands are processed sequentially,
//! so `TickEngine` needs no locks. The thread does not belong to tokio: plugin calls
//! (synchronous wasmtime-wasi) cannot be made inside an async context. Network tasks
//! are spawned by the engine onto the server runtime via `HostState::runtime`.

use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ticknode_core::{EngineError, RemovedConnections, TickEngine};
use ticknode_shared::{ClientMessage, NodeId, NodeInstance, NodeTrace, NodeSize, Position, ServerMessage, UiMessage};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{error, info};

use crate::plugins::Plugins;
use crate::storage::Storage;

const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);
/// A larger fragment is almost certainly a bug or abuse.
const MAX_PASTE_NODES: usize = 1000;

pub enum Command {
    Client { msg: ClientMessage, reply: mpsc::UnboundedSender<ServerMessage> },
    Snapshot(oneshot::Sender<ServerMessage>),
    Shutdown(oneshot::Sender<()>),
}

/// Handle to the engine actor for HTTP/WebSocket handlers.
#[derive(Clone)]
pub struct EngineHandle {
    commands: std_mpsc::Sender<Command>,
    events: broadcast::Sender<Bytes>,
}

impl EngineHandle {
    /// Events broadcast to all editors, pre-encoded with postcard.
    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.events.subscribe()
    }

    pub fn send(&self, msg: ClientMessage, reply: mpsc::UnboundedSender<ServerMessage>) {
        let _ = self.commands.send(Command::Client { msg, reply });
    }

    pub async fn snapshot(&self) -> Option<ServerMessage> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Snapshot(tx)).ok()?;
        rx.await.ok()
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
    layout: crate::storage::Layout,
    storage: Storage,
    plugins: Plugins,
    tick: Duration,
) -> std::io::Result<EngineHandle> {
    let (positions, sizes, routes) = layout;
    let (commands, rx) = std_mpsc::channel();
    let (events, _) = broadcast::channel(1024);
    engine.host_state.runtime = Some(tokio::runtime::Handle::current());
    engine.host_state.tick_duration = tick;
    let actor = Actor { engine, positions, sizes, routes, storage, plugins, events: events.clone(), dirty_since: None, traces: HashMap::new(), history: Default::default(), telemetry: telemetry::Telemetry::new(Instant::now()) };
    std::thread::Builder::new()
        .name("ticknode-engine".into())
        .spawn(move || actor.run(rx, tick))?;
    Ok(EngineHandle { commands, events })
}

struct Actor {
    engine: TickEngine,
    history: history::History,
    positions: HashMap<NodeId, Position>,
    sizes: HashMap<NodeId, NodeSize>,
    routes: routes::Routes,
    storage: Storage,
    plugins: Plugins,
    events: broadcast::Sender<Bytes>,
    /// When the graph was first modified since the last save.
    dirty_since: Option<Instant>,
    /// Latest execution of each node (in-memory only: runtime state, not part of the graph).
    traces: HashMap<NodeId, (NodeTrace, Instant)>,
    /// Traces and logs waiting for the next telemetry interval.
    telemetry: telemetry::Telemetry,
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
            Command::Client { msg, reply } => match self.apply(msg, &reply) {
                Ok((events, created)) => {
                    events.iter().for_each(|e| self.broadcast(e));
                    // Enabling, disabling or configuring a plugin may change its options
                    self.broadcast_option_changes();
                    // Sent after broadcast: `NodeAdded` is already in the event channel, and the ws session delivers it first
                    if let Some(ids) = created {
                        let _ = reply.send(ServerMessage::Created(ids));
                    }
                }
                Err(e) => { let _ = reply.send(ServerMessage::Error(e)); }
            },
            Command::Snapshot(tx) => { let _ = tx.send(self.snapshot()); }
            Command::Shutdown(done) => {
                self.save();
                info!("Engine stopped");
                let _ = done.send(());
                return true;
            }
        }
        false
    }

    /// Broadcasts the dynamic property options that plugins changed since the last check.
    /// Unchanged lists never appear here (the engine tracks changes), so editors are not spammed.
    fn broadcast_option_changes(&mut self) {
        for set in self.engine.take_property_option_changes() {
            self.broadcast(&ServerMessage::PropertyOptions { plugin: set.plugin, source: set.source, options: set.options });
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
        for message in self.telemetry.flush(now, &mut self.engine) {
            self.broadcast(&message);
        }
    }

    fn broadcast(&self, msg: &ServerMessage) {
        // An error simply means there are no active subscribers.
        let _ = self.events.send(encode(msg));
    }

    fn save(&mut self) {
        if self.dirty_since.is_none() {
            return;
        }
        match self.storage.save(&self.engine, &self.positions, &self.sizes, &self.routes) {
            Ok(()) => self.dirty_since = None,
            Err(e) => {
                error!("Failed to save the graph: {:#}", e);
                // Retry after the next debounce interval.
                self.dirty_since = Some(Instant::now());
            }
        }
    }

    fn instance(&self, id: NodeId) -> Option<NodeInstance> {
        let mut model = self.engine.node_model(id)?;
        model.position = self.positions.get(&id).copied().unwrap_or_default();
        model.size = self.sizes.get(&id).copied();
        Some(NodeInstance { model, definition: self.engine.node_definition(id)?, unavailable: self.engine.node_unavailable(id), frozen: self.engine.frozen_changes(id).to_vec() })
    }

    fn snapshot(&self) -> ServerMessage {
        let graph = self.engine.export_graph();
        ServerMessage::Init {
            definitions: self.engine.registry.get_definitions(),
            types: self.engine.types.definitions(),
            plugins: self.plugins.infos(&self.engine),
            translations: self.engine.translations(),
            property_options: self.engine.property_options(),
            history: self.history.state(),
            wire_routes: routes::sorted(&self.routes),
            detached_connections: graph.detached_connections,
            timers: self.engine.delay_timers(),
            node_errors: self.engine.node_errors(),
            nodes: graph.nodes.iter().filter_map(|n| self.instance(n.id)).collect(),
            exec_connections: graph.exec_connections,
            data_connections: graph.data_connections,
            traces: self.traces.values()
                .map(|(trace, at)| (trace.clone(), at.elapsed().as_millis() as u64))
                .collect(),
        }
    }

    fn catalog(&self) -> ServerMessage {
        ServerMessage::Catalog {
            definitions: self.engine.registry.get_definitions(),
            types: self.engine.types.definitions(),
            translations: self.engine.translations(),
        }
    }

    fn require_node(&self, id: NodeId) -> Result<(), UiMessage> {
        if self.engine.has_node(id) { Ok(()) } else { Err(EngineError::NodeNotFound(id).ui_message()) }
    }
}

pub(crate) mod routes;
mod commands;
mod history;
mod telemetry;
mod settings;
mod resize;

#[cfg(test)]
mod tests;
