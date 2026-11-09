use super::*;
use ticknode_shared::DataValue;

impl Actor {
    pub(super) fn paste_settings(
        &mut self, id: NodeId, type_name: &str, properties: Vec<(String, DataValue)>,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        let sheet = self.node_sheet.get(&id).copied().ok_or_else(|| EngineError::NodeNotFound(id).ui_message())?;
        if let Some(reason) = self.engine.node_unavailable(id) {
            return Err(EngineError::PluginUnavailable(reason).ui_message());
        }
        let mut target = self.graph_snapshot();
        let node = target.nodes.iter_mut().find(|node| node.id == id).expect("the node exists");
        if node.type_name != type_name {
            return Err(UiMessage::new(
                "error-settings-type-mismatch",
                format!("Cannot paste settings from {type_name} to {}", node.type_name),
            ).arg("from", type_name).arg("to", &node.type_name));
        }
        if node.properties == properties {
            self.engine.reconcile_settings(&target, id);
            return Ok(vec![]);
        }
        node.properties = properties;
        let tagged = self.reconcile_graph_edit_for_sheet(&target, Some(id), sheet);
        Ok(tagged.into_iter().map(|(_, e)| e).collect())
    }
}
