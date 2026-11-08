//! Apply server graph, catalog and telemetry messages to editor signals.

use super::*;

impl Editor {
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
            ServerMessage::Init { definitions, nodes, exec_connections, data_connections, traces, types, plugins, translations, property_options } => {
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
                self.property_options.set(property_options.into_iter().map(|s| ((s.plugin, s.source), Arc::new(s.options))).collect());
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
            ServerMessage::PropertyOptions { plugin, source, options } => self.property_options.update(|all| {
                if options.is_empty() {
                    all.remove(&(plugin, source));
                } else {
                    all.insert((plugin, source), Arc::new(options));
                }
            }),
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
