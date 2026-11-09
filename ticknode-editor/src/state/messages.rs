//! Apply server graph, catalog and telemetry messages to editor signals.

use super::*;

mod connections;
mod snapshot;
mod routes;
mod detached;

impl Editor {
    pub fn apply(&self, msg: ServerMessage) {
        if timers::applies(&msg) { self.timers.update(|all| timers::apply(all, &msg, now_ms())); }
        if routes::applies(&msg) { self.wire_routes.update(|all| routes::apply(all, &msg)); }
        if detached::applies(&msg) { self.detached.update(|all| detached::apply(all, &msg)); }
        match msg {
            ServerMessage::Init { definitions, nodes, exec_connections, data_connections, traces, types, plugins, translations, property_options, history, .. } => {
                self.history.set(history);
                let gone = self.remove_absent_nodes(&nodes);
                self.apply_catalog(definitions, types, translations);
                self.plugins.set(plugins);
                self.property_options.set(property_options.into_iter().map(|s| ((s.plugin, s.source), Arc::new(s.options))).collect());
                self.replace_graph(nodes, exec_connections, data_connections, traces, gone);
            }
            ServerMessage::NodeAdded(instance) | ServerMessage::NodeUpdated(instance) => { self.upsert_node(instance); }
            ServerMessage::NodeRemoved(id) => self.remove_node(id),
            ServerMessage::NodeMoved { id, position } => self.move_node(id, position),
            ServerMessage::ExecConnected(c) => self.exec.update(|all| connections::connect_exec(all, c)),
            ServerMessage::ExecDisconnected(c) => {
                self.exec.update(|all| all.retain(|x| *x != c));
                self.prune_selection();
            }
            ServerMessage::DataConnected(c) => self.data.update(|all| connections::connect_data(all, c)),
            ServerMessage::DataDisconnected { dst_node, dst_pin } => {
                self.data.update(|all| connections::disconnect_data(all, dst_node, dst_pin));
                self.prune_selection();
            }
            ServerMessage::WireRouted(route) => {
                let key = (route.exec, route.connection);
                if self.selected_point.get_untracked().is_some_and(|(k, i)| k == key && i >= route.points.len()) { self.selected_point.set(None); }
            }
            ServerMessage::Detached(_) | ServerMessage::Timers(_) => {},
            ServerMessage::DetachedRemoved(_) => self.prune_selection(),
            ServerMessage::Executed(traces) => self.apply_traces(traces),
            ServerMessage::PluginUpdated(info) => self.plugins.update(|plugins| update_plugin(plugins, info)),
            // Plugin enabled or disabled: refresh palette and type colors from the updated catalog.
            ServerMessage::Catalog { definitions, types, translations } => self.apply_catalog(definitions, types, translations),
            ServerMessage::PropertyOptions { plugin, source, options } => self.property_options.update(|all| update_options(all, plugin, source, options)),
            // Created follows the NodeAdded messages for our AddNode / AddNodeConnected / Paste request.
            ServerMessage::Created(ids) => self.select_nodes(ids),
            ServerMessage::Log { node_id, message } => self.node_log(node_id, message),
            ServerMessage::History(history) => self.history.set(history),
            ServerMessage::Error(message) => {
                self.error_seq.update(|n| *n += 1);
                // The answer to the user's action: translated, unlike technical log lines.
                self.log(LogKind::Error, format!("[ERROR] {}", i18n::ui_text(&message)));
            }
        }
    }

    fn upsert_node(&self, instance: NodeInstance) -> NodeId {
        let id = instance.model.id;
        match self.node(id) {
            Some(existing) => {
                existing.pos.set(instance.model.position);
                existing.instance.set(instance);
            }
            None => self.nodes.update(|nodes| { nodes.insert(id, ClientNode::new(instance)); }),
        }
        id
    }

    fn remove_node(&self, id: NodeId) {
        let removed = self.nodes.try_update(|nodes| nodes.remove(&id)).flatten();
        self.exec.update(|e| e.retain(|c| c.src_node != id && c.dst_node != id));
        self.data.update(|d| d.retain(|c| c.src_node != id && c.dst_node != id));
        self.prune_selection();
        dispose_later(removed);
    }

    fn move_node(&self, id: NodeId, position: Position) {
        if let Some(node) = self.node(id) {
            if node.pos.get_untracked() != position { node.pos.set(position); }
        }
    }

    fn apply_catalog(&self, definitions: Vec<NodeDefinition>, types: Vec<TypeDefinition>, translations: Translations) {
        i18n::set_runtime_translations(&translations);
        let registry = TypeRegistry::from_definitions(types);
        i18n::set_known_types(&registry);
        self.types.set(registry);
        self.definitions.set(definitions);
    }

    fn apply_traces(&self, traces: Vec<NodeTrace>) {
        let now = now_ms();
        for trace in traces {
            if let Some(node) = self.node(trace.node_id) { node.set_trace(trace, now); }
        }
    }

    fn node_log(&self, node_id: NodeId, message: String) {
        // Logs stay English: use the definition title, not a translation.
        let title = self.node(node_id)
            .map(|n| n.instance.with_untracked(|i| i.definition.title.clone()))
            .unwrap_or_else(|| "?".to_string());
        self.log(LogKind::Node, format!("[{title}] {message}"));
    }
}

fn update_plugin(plugins: &mut Vec<PluginInfo>, info: PluginInfo) {
    match plugins.iter_mut().find(|p| p.name == info.name) {
        Some(existing) => *existing = info,
        None => {
            plugins.push(info);
            plugins.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }
}

fn update_options(all: &mut PropertyOptions, plugin: String, source: String, options: Vec<PropertyOption>) {
    if options.is_empty() { all.remove(&(plugin, source)); }
    else { all.insert((plugin, source), Arc::new(options)); }
}

#[cfg(test)]
mod tests;
