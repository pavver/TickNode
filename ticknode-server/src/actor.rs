//! The engine lives on a dedicated thread (actor): ticks and editor commands are processed sequentially,
//! so `TickEngine` needs no locks. The thread does not belong to tokio: plugin calls
//! (synchronous wasmtime-wasi) cannot be made inside an async context. Network tasks
//! are spawned by the engine onto the server runtime via `HostState::runtime`.

use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ticknode_core::{EngineError, RemovedConnections, TickEngine};
use ticknode_shared::{ClientMessage, NodeId, NodeInstance, NodeTrace, Position, ServerMessage, UiMessage};
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
    positions: HashMap<NodeId, Position>,
    storage: Storage,
    plugins: Plugins,
    tick: Duration,
) -> std::io::Result<EngineHandle> {
    let (commands, rx) = std_mpsc::channel();
    let (events, _) = broadcast::channel(1024);
    engine.host_state.runtime = Some(tokio::runtime::Handle::current());
    engine.host_state.tick_duration = tick;
    let actor = Actor { engine, positions, storage, plugins, events: events.clone(), dirty_since: None, traces: HashMap::new() };
    std::thread::Builder::new()
        .name("ticknode-engine".into())
        .spawn(move || actor.run(rx, tick))?;
    Ok(EngineHandle { commands, events })
}

struct Actor {
    engine: TickEngine,
    positions: HashMap<NodeId, Position>,
    storage: Storage,
    plugins: Plugins,
    events: broadcast::Sender<Bytes>,
    /// When the graph was first modified since the last save.
    dirty_since: Option<Instant>,
    /// Latest execution of each node (in-memory only: runtime state, not part of the graph).
    traces: HashMap<NodeId, (NodeTrace, Instant)>,
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
                Ok(Command::Client { msg, reply }) => match self.apply(msg, &reply) {
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
                Ok(Command::Snapshot(tx)) => { let _ = tx.send(self.snapshot()); }
                Ok(Command::Shutdown(done)) => {
                    self.save();
                    info!("Engine stopped");
                    let _ = done.send(());
                    return;
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    self.save();
                    return;
                }
            }
        }
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
            return;
        }
        if !self.engine.executed.is_empty() {
            self.broadcast(&ServerMessage::Executed(self.engine.executed.clone()));
        }
        for (node_id, message) in std::mem::take(&mut self.engine.host_state.logs) {
            self.broadcast(&ServerMessage::Log { node_id, message });
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
        match self.storage.save(&self.engine, &self.positions) {
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
        Some(NodeInstance { model, definition: self.engine.node_definition(id)?, unavailable: self.engine.node_unavailable(id) })
    }

    fn snapshot(&self) -> ServerMessage {
        let graph = self.engine.export_graph();
        ServerMessage::Init {
            definitions: self.engine.registry.get_definitions(),
            types: self.engine.types.definitions(),
            plugins: self.plugins.infos(&self.engine),
            translations: self.engine.translations(),
            property_options: self.engine.property_options(),
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

    /// Applies an editor command; returns events to broadcast to all editors and the IDs of nodes
    /// created for the initiator (`ServerMessage::Created`).
    fn apply(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(Vec<ServerMessage>, Option<Vec<NodeId>>), UiMessage> {
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. } | ClientMessage::SetPluginEnabled { .. } | ClientMessage::ConfigurePlugin { .. });
        let mut created = None;
        let events = match msg {
            ClientMessage::AddNode { type_name, position } => {
                let id = NodeId::generate();
                self.engine.create_node(id, &type_name, &[]).map_err(|e| e.ui_message())?;
                self.positions.insert(id, position);
                created = Some(vec![id]);
                vec![ServerMessage::NodeAdded(self.instance(id).expect("the node was just created"))]
            }
            ClientMessage::RemoveNode(id) => {
                self.require_node(id)?;
                self.engine.remove_node(id);
                self.positions.remove(&id);
                self.traces.remove(&id);
                vec![ServerMessage::NodeRemoved(id)]
            }
            ClientMessage::MoveNode { id, position } => {
                self.require_node(id)?;
                self.positions.insert(id, position);
                vec![ServerMessage::NodeMoved { id, position }]
            }
            ClientMessage::SetProperty { id, name, value } => {
                let removed = self.engine.set_node_property(id, &name, value).map_err(|e| e.ui_message())?;
                let mut events = removed_events(removed);
                events.push(ServerMessage::NodeUpdated(self.instance(id).expect("the node exists")));
                events
            }
            ClientMessage::SetPinSplit { node, pin, split } => {
                let removed = self.engine.set_pin_split(node, pin, split).map_err(|e| e.ui_message())?;
                let mut events = removed_events(removed);
                events.push(ServerMessage::NodeUpdated(self.instance(node).expect("the node exists")));
                events
            }
            ClientMessage::SetInputValue { id, pin, value } => {
                self.engine.set_input_value(id, pin, value).map_err(|e| e.ui_message())?;
                vec![ServerMessage::NodeUpdated(self.instance(id).expect("the node exists"))]
            }
            ClientMessage::ConnectExec(c) => {
                self.engine.connect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
                vec![ServerMessage::ExecConnected(c)]
            }
            ClientMessage::DisconnectExec(c) => {
                if self.engine.disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin) {
                    vec![ServerMessage::ExecDisconnected(c)]
                } else {
                    vec![]
                }
            }
            ClientMessage::ConnectData(c) => {
                self.engine.connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
                vec![ServerMessage::DataConnected(c)]
            }
            ClientMessage::DisconnectData { dst_node, dst_pin } => {
                if self.engine.disconnect_data(dst_node, dst_pin) {
                    vec![ServerMessage::DataDisconnected { dst_node, dst_pin }]
                } else {
                    vec![]
                }
            }
            ClientMessage::TriggerNode { id, pin } => {
                self.require_node(id)?;
                self.engine.trigger_node(id, pin);
                vec![]
            }
            ClientMessage::Paste(snippet) => {
                if snippet.nodes.len() > MAX_PASTE_NODES {
                    return Err(UiMessage::new(
                        "error-paste-too-many",
                        format!("Too many nodes to paste: {} (maximum {})", snippet.nodes.len(), MAX_PASTE_NODES),
                    ).arg("count", snippet.nodes.len()).arg("max", MAX_PASTE_NODES));
                }
                let (imported, warnings) = self.engine.import_snippet(&snippet);
                if !warnings.is_empty() {
                    let details = warnings.join("; ");
                    let _ = reply.send(ServerMessage::Error(
                        UiMessage::new("error-paste-incomplete", format!("Not everything was pasted: {details}")).arg("details", details),
                    ));
                }
                let positions: HashMap<NodeId, Position> = snippet.nodes.iter().map(|n| (n.id, n.position)).collect();
                let ids: Vec<NodeId> = imported.iter().map(|(_, new)| *new).collect();
                for (old, new) in &imported {
                    self.positions.insert(*new, positions.get(old).copied().unwrap_or_default());
                }
                let mut events: Vec<ServerMessage> = ids.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeAdded).collect();
                let inner = self.engine.export_snippet(&ids);
                events.extend(inner.exec_connections.into_iter().map(ServerMessage::ExecConnected));
                events.extend(inner.data_connections.into_iter().map(ServerMessage::DataConnected));
                created = Some(ids);
                events
            }
            ClientMessage::SetPluginEnabled { name, enabled } => {
                let outcome = self.plugins.set_enabled(&mut self.engine, &name, enabled)?;
                if let Some(warning) = outcome.warning {
                    let _ = reply.send(ServerMessage::Error(warning));
                }
                // Send new definitions first so the editor knows the pins of converted nodes
                let mut events = vec![self.catalog()];
                events.extend(outcome.change.replaced.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeUpdated));
                if !outcome.change.removed.exec.is_empty() || !outcome.change.removed.data.is_empty() {
                    self.dirty_since.get_or_insert_with(Instant::now);
                }
                events.extend(removed_events(outcome.change.removed));
                events.extend(self.plugins.info(&self.engine, &name).map(ServerMessage::PluginUpdated));
                events
            }
            ClientMessage::ConfigurePlugin { name, values } => {
                self.plugins.configure(&mut self.engine, &name, values)?;
                self.plugins.info(&self.engine, &name).map(ServerMessage::PluginUpdated).into_iter().collect()
            }
        };
        if mutates && !events.is_empty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok((events, created))
    }
}

fn removed_events(removed: RemovedConnections) -> Vec<ServerMessage> {
    let exec = removed.exec.into_iter().map(ServerMessage::ExecDisconnected);
    let data = removed.data.into_iter().map(|c| ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin });
    exec.chain(data).collect()
}

#[cfg(test)]
mod split_tests {
    use super::*;
    use ticknode_shared::*;

    #[test]
    fn split_protocol_round_trip_updates_effective_pins_and_disconnects() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = TickEngine::new();
        engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
            FieldDefinition::new("x", "X", DataType::Number),
        ])], &[]).unwrap();
        let source = NodeId::generate();
        let sink = NodeId::generate();
        engine.create_node(source, "type/make/test.point", &[]).unwrap();
        engine.create_node(sink, "type/break/test.point", &[]).unwrap();
        engine.connect_data(source, PinId(0), sink, PinId(0)).unwrap();
        let (events, _) = broadcast::channel(16);
        let mut actor = Actor {
            engine, positions: HashMap::new(), storage: Storage::new(dir.path().join("graph.json")),
            plugins: Plugins::new(dir.path()), events, dirty_since: None, traces: HashMap::new(),
        };
        for split in [true, false] {
            let msg = ClientMessage::SetPinSplit { node: source, pin: PinId(0), split };
            let decoded = postcard::from_bytes(&postcard::to_allocvec(&msg).unwrap()).unwrap();
            assert_eq!(msg, decoded);
            let (reply, _) = mpsc::unbounded_channel();
            let (events, _) = actor.apply(decoded, &reply).unwrap();
            if split { assert!(events.iter().any(|m| matches!(m, ServerMessage::DataDisconnected { dst_node, .. } if *dst_node == sink))); }
            let event = events.last().unwrap();
            let decoded: ServerMessage = postcard::from_bytes(&encode(event)).unwrap();
            assert_eq!(*event, decoded);
            let ServerMessage::NodeUpdated(instance) = decoded else { panic!("missing NodeUpdated") };
            assert!(instance.definition.pins.iter().any(|p| p.id == if split { split_field_pin(PinId(0), 0).unwrap() } else { PinId(0) } && p.direction == PinDirection::Output));
        }
    }
}
