//! Node size validation and layout reconciliation, independent of engine runtime state.
use super::*;
use ticknode_shared::GraphModel;

impl Actor {
    pub(super) fn resize_node(&mut self, id: NodeId, size: Option<NodeSize>) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_node(id)?;
        let size = size.map(|size| size.validated().ok_or_else(||
            UiMessage::new("error-node-size", "Node dimensions must be finite")
        )).transpose()?;
        if self.sizes.get(&id).copied() == size { return Ok(vec![]); }
        match size {
            Some(size) => { self.sizes.insert(id, size); }
            None => { self.sizes.remove(&id); }
        }
        Ok(vec![ServerMessage::NodeResized { id, size }])
    }

    pub(super) fn restore_sizes(&mut self, target: &GraphModel, added: &[NodeId]) -> Vec<ServerMessage> {
        let mut events = Vec::new();
        for node in &target.nodes {
            if !self.engine.has_node(node.id) { continue; }
            let size = node.size.and_then(NodeSize::validated);
            let old = match size {
                Some(size) => self.sizes.insert(node.id, size),
                None => self.sizes.remove(&node.id),
            };
            if !added.contains(&node.id) && old != size {
                events.push(ServerMessage::NodeResized { id: node.id, size });
            }
        }
        events
    }
}
