use super::*;
use ticknode_core::TickBudget;
use ticknode_shared::ServerSettings;

impl Actor {
    pub(super) fn set_server_settings(
        &mut self,
        settings: ServerSettings,
    ) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        let settings = settings.validated();
        self.server_settings = settings;
        let tick = Duration::from_millis(settings.tick_ms as u64);
        self.engine.host_state.tick_duration = tick;
        self.engine.budget = TickBudget { limit: (settings.tick_budget != 0).then_some(settings.tick_budget) };
        self.storage.save_settings(&settings).map_err(|e| {
            tracing::error!("Failed to save settings: {e:#}");
            UiMessage::new("error-save-settings", "Failed to save settings")
        })?;
        Ok(vec![(None, ServerMessage::ServerSettingsChanged(settings))])
    }
}
