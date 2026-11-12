use super::*;
use ticknode_core::TickBudget;
use ticknode_shared::ServerSettings;

impl Actor {
    pub(super) fn set_server_settings(
        &mut self,
        settings: ServerSettings,
    ) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        settings.display.validate()?;
        if !["en", "uk"].contains(&settings.locale.as_str()) {
            tracing::warn!(locale = %settings.locale, "Invalid formatting locale; using en");
        }
        let settings = settings.validated();
        self.engine.set_locale(&settings.locale);
        self.engine.set_display_rules(settings.display.clone());
        self.server_settings = settings.clone();
        let tick = Duration::from_millis(settings.tick_ms as u64);
        self.engine.host_state.tick_duration = tick;
        self.engine.budget = TickBudget {
            limit: (settings.tick_budget != 0).then_some(settings.tick_budget),
        };
        self.storage.save_settings(&settings).map_err(|e| {
            tracing::error!("Failed to save settings: {e:#}");
            UiMessage::new("error-save-settings", "Failed to save settings")
        })?;
        Ok(vec![(None, ServerMessage::ServerSettingsChanged(settings))])
    }
}
