//! Plugin enable/disable and settings commands.

use super::*;
use ticknode_shared::SheetId;

impl Actor {
    pub(super) fn configure_plugin(
        &mut self,
        name: &str,
        values: Vec<(String, ticknode_shared::SettingUpdate)>,
    ) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        self.plugins.configure(&mut self.engine, name, values)?;
        Ok(self
            .plugins
            .info(&self.engine, name)
            .map(|i| (None, ServerMessage::PluginUpdated(i)))
            .into_iter()
            .collect())
    }

    pub(super) fn set_plugin_enabled(
        &mut self,
        name: &str,
        enabled: bool,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        let outcome = self.plugins.set_enabled(&mut self.engine, name, enabled)?;
        self.histories.clear();
        if let Some(warning) = outcome.warning {
            let _ = reply.send(ServerMessage::Error(warning));
        }
        let mut events = vec![(None, self.catalog())];
        for id in &outcome.change.replaced {
            if let Some(inst) = self.instance(*id) {
                let sheet = self.node_sheet.get(id).copied();
                events.push((sheet, ServerMessage::NodeUpdated(inst)));
            }
        }
        if !outcome.change.replaced.is_empty()
            || !outcome.change.detached.is_empty()
            || !outcome.change.removed.exec.is_empty()
            || !outcome.change.removed.data.is_empty()
        {
            self.dirty_since.get_or_insert_with(Instant::now);
            self.dirty_sheets.extend(self.sheets.iter().map(|s| s.id));
        }
        for c in outcome.change.removed.exec {
            let sheet = self.node_sheet.get(&c.src_node).copied();
            events.push((sheet, ServerMessage::ExecDisconnected(c)));
        }
        for c in outcome.change.removed.data {
            let sheet = self.node_sheet.get(&c.dst_node).copied();
            events.push((
                sheet,
                ServerMessage::DataDisconnected {
                    dst_node: c.dst_node,
                    dst_pin: c.dst_pin,
                },
            ));
        }
        for d in outcome.change.detached {
            let sheet = self.node_sheet.get(&d.connection.src_node).copied();
            events.push((sheet, ServerMessage::Detached(d)));
        }
        if let Some(info) = self.plugins.info(&self.engine, name) {
            events.push((None, ServerMessage::PluginUpdated(info)));
        }
        for sheet in &self.sheets {
            events.push((
                Some(sheet.id),
                ServerMessage::History(ticknode_shared::HistoryState::default()),
            ));
        }
        Ok(events)
    }
}
