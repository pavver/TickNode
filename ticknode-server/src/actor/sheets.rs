use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::Actor;
use ticknode_shared::{
    ConnectionModel, FrameId, GraphModel, MAX_SHEETS, NodeId, NodeSize, ServerMessage, SheetId,
    SheetInfo, SheetSummary, UiMessage,
};
use tokio::sync::mpsc;

impl Actor {
    pub(super) fn sheet_graph(&self, sheet_id: SheetId) -> GraphModel {
        let mut graph = self.engine.export_graph();
        graph
            .nodes
            .retain(|n| self.node_sheet.get(&n.id) == Some(&sheet_id));
        for node in &mut graph.nodes {
            node.position = self.positions.get(&node.id).copied().unwrap_or_default();
            node.size = self.sizes.get(&node.id).copied();
        }
        graph
            .exec_connections
            .retain(|c| self.node_sheet.get(&c.src_node) == Some(&sheet_id));
        graph
            .data_connections
            .retain(|c| self.node_sheet.get(&c.src_node) == Some(&sheet_id));
        graph.wire_routes = super::routes::sorted(&self.routes)
            .into_iter()
            .filter(|r| self.node_sheet.get(&r.connection.src_node) == Some(&sheet_id))
            .collect();
        graph
            .detached_connections
            .retain(|d| self.node_sheet.get(&d.connection.src_node) == Some(&sheet_id));
        graph.frames = self
            .frames
            .values()
            .filter(|f| self.frame_sheet.get(&f.id) == Some(&sheet_id))
            .cloned()
            .collect();
        graph
    }

    pub(super) fn sheet_summaries(&self) -> Vec<SheetSummary> {
        let node_errors = self.engine.node_errors();
        self.sheets
            .iter()
            .map(|sheet| {
                let nodes = self.node_sheet.values().filter(|s| **s == sheet.id).count() as u32;
                let errors = node_errors
                    .iter()
                    .filter(|e| self.node_sheet.get(&e.node) == Some(&sheet.id))
                    .count() as u32;
                let last_executed_ms = self
                    .traces
                    .iter()
                    .filter(|(node_id, _)| self.node_sheet.get(node_id) == Some(&sheet.id))
                    .map(|(_, (_, at))| at.elapsed().as_millis() as u64)
                    .min();
                SheetSummary {
                    info: sheet.clone(),
                    nodes,
                    errors,
                    last_executed_ms,
                }
            })
            .collect()
    }

    pub(super) fn broadcast_sheets(&mut self) {
        let summaries = self.sheet_summaries();
        self.broadcast(None, &ServerMessage::Sheets(summaries.clone()));
        self.last_sheet_summaries = summaries;
        self.last_sheets_broadcast = Instant::now();
    }

    pub(super) fn check_sheets_broadcast(&mut self, now: Instant) {
        if now < self.last_sheets_broadcast + Duration::from_secs(2) {
            return;
        }
        let current = self.sheet_summaries();
        let changed = current.len() != self.last_sheet_summaries.len()
            || current
                .iter()
                .zip(&self.last_sheet_summaries)
                .any(|(c, old)| {
                    c.info != old.info
                        || c.nodes != old.nodes
                        || c.errors != old.errors
                        || self.traces.iter().any(|(id, (_, at))| {
                            self.node_sheet.get(id) == Some(&c.info.id)
                                && *at > self.last_sheets_broadcast
                        })
                });

        if changed {
            self.broadcast(None, &ServerMessage::Sheets(current.clone()));
            self.last_sheet_summaries = current;
            self.last_sheets_broadcast = now;
        }
    }

    pub(super) fn create_sheet(
        &mut self,
        title: String,
        caption: String,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<SheetId, UiMessage> {
        if self.sheets.len() >= MAX_SHEETS {
            return Err(UiMessage::new("error-sheet-limit", "Too many sheets"));
        }
        let id = SheetId::generate();
        let info = SheetInfo {
            id,
            title,
            caption,
            enabled: true,
        }
        .validated();
        self.sheets.push(info);
        self.dirty_sheets_meta = true;
        self.dirty_sheets.insert(id);
        self.dirty_since.get_or_insert_with(Instant::now);
        self.broadcast_sheets();
        let _ = reply.send(ServerMessage::SheetCreated(id));
        Ok(id)
    }

    pub(super) fn update_sheet(&mut self, info: SheetInfo) -> Result<(), UiMessage> {
        let validated = info.validated();
        let idx = self
            .sheets
            .iter()
            .position(|s| s.id == validated.id)
            .ok_or_else(|| UiMessage::new("error-sheet-not-found", "Sheet not found"))?;
        let enabled_changed = self.sheets[idx].enabled != validated.enabled;
        self.sheets[idx] = validated.clone();
        self.dirty_sheets_meta = true;
        self.dirty_since.get_or_insert_with(Instant::now);

        if enabled_changed {
            let nodes: Vec<NodeId> = self
                .node_sheet
                .iter()
                .filter(|(_, s)| **s == validated.id)
                .map(|(n, _)| *n)
                .collect();
            self.engine.set_disabled(nodes, !validated.enabled);
        }
        self.broadcast_sheets();
        Ok(())
    }

    pub(super) fn delete_sheet(&mut self, id: SheetId) -> Result<(), UiMessage> {
        let idx = self
            .sheets
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| UiMessage::new("error-sheet-not-found", "Sheet not found"))?;
        self.sheets.remove(idx);

        let nodes_to_remove: Vec<NodeId> = self
            .node_sheet
            .iter()
            .filter(|(_, s)| **s == id)
            .map(|(n, _)| *n)
            .collect();
        for node_id in &nodes_to_remove {
            self.engine.remove_node(*node_id);
            self.positions.remove(node_id);
            self.sizes.remove(node_id);
            self.traces.remove(node_id);
            self.node_sheet.remove(node_id);
        }

        let frames_to_remove: Vec<FrameId> = self
            .frame_sheet
            .iter()
            .filter(|(_, s)| **s == id)
            .map(|(f, _)| *f)
            .collect();
        for frame_id in &frames_to_remove {
            self.frames.remove(frame_id);
            self.frame_sheet.remove(frame_id);
        }

        self.routes.retain(|(_, c), _| {
            !nodes_to_remove.contains(&c.src_node) && !nodes_to_remove.contains(&c.dst_node)
        });
        self.histories.remove(&id);
        self.engine.variables.remove_sheet(id);
        let _ = self.storage.delete_sheet(id);
        self.dirty_sheets.remove(&id);
        self.dirty_sheets_meta = true;
        self.dirty_since.get_or_insert_with(Instant::now);

        // Notify sessions with this sheet open
        self.broadcast(Some(id), &self.snapshot(None));
        self.broadcast_sheets();
        Ok(())
    }

    pub(super) fn move_sheet(&mut self, id: SheetId, index: u32) -> Result<(), UiMessage> {
        let old_idx = self
            .sheets
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| UiMessage::new("error-sheet-not-found", "Sheet not found"))?;
        let sheet = self.sheets.remove(old_idx);
        let new_idx = (index as usize).min(self.sheets.len());
        self.sheets.insert(new_idx, sheet);
        self.dirty_sheets_meta = true;
        self.dirty_since.get_or_insert_with(Instant::now);
        self.broadcast_sheets();
        Ok(())
    }

    pub(super) fn duplicate_sheet(
        &mut self,
        id: SheetId,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<SheetId, UiMessage> {
        let original_idx = self
            .sheets
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| UiMessage::new("error-sheet-not-found", "Sheet not found"))?;
        let original = &self.sheets[original_idx];
        let new_id = SheetId::generate();
        let new_title = format!("{} (copy)", original.title);
        let new_info = SheetInfo {
            id: new_id,
            title: new_title,
            caption: original.caption.clone(),
            enabled: original.enabled,
        }
        .validated();
        self.sheets.insert(original_idx + 1, new_info.clone());

        let snippet = self.sheet_graph(id);
        let (imported, _) = self.engine.import_snippet(&snippet);
        let mapping: HashMap<NodeId, NodeId> = imported.iter().copied().collect();

        for (old_id, new_node_id) in &imported {
            self.node_sheet.insert(*new_node_id, new_id);
            self.engine.set_node_sheet(*new_node_id, Some(new_id));
            if let Some(pos) = snippet
                .nodes
                .iter()
                .find(|n| n.id == *old_id)
                .map(|n| n.position)
            {
                self.positions.insert(*new_node_id, pos);
            }
            if let Some(size) = snippet
                .nodes
                .iter()
                .find(|n| n.id == *old_id)
                .and_then(|n| n.size)
                .and_then(NodeSize::validated)
            {
                self.sizes.insert(*new_node_id, size);
            }
        }

        if !new_info.enabled {
            let new_nodes = imported.iter().map(|(_, n)| *n);
            self.engine.set_disabled(new_nodes, true);
        }

        for route in &snippet.wire_routes {
            if let (Some(&new_src), Some(&new_dst)) = (
                mapping.get(&route.connection.src_node),
                mapping.get(&route.connection.dst_node),
            ) {
                let conn = ConnectionModel {
                    src_node: new_src,
                    src_pin: route.connection.src_pin,
                    dst_node: new_dst,
                    dst_pin: route.connection.dst_pin,
                };
                self.routes.insert((route.exec, conn), route.points.clone());
            }
        }

        for frame in &snippet.frames {
            let new_frame_id = FrameId::generate();
            let mut new_frame = frame.clone();
            new_frame.id = new_frame_id;
            self.frames.insert(new_frame_id, new_frame);
            self.frame_sheet.insert(new_frame_id, new_id);
        }

        self.dirty_sheets_meta = true;
        self.dirty_sheets.insert(new_id);
        self.dirty_since.get_or_insert_with(Instant::now);
        self.broadcast_sheets();
        let _ = reply.send(ServerMessage::SheetCreated(new_id));
        Ok(new_id)
    }
}
