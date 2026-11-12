use ticknode_shared::{MAX_TICK_BUDGET, MAX_TICK_MS, MIN_TICK_BUDGET, MIN_TICK_MS, ServerSettings};

/// Draft representation of server-wide engine settings edited in the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineSettingsDraft {
    pub locale: String,
    pub display: super::display::logic::DisplayDraft,
    pub tick_ms: String,
    pub tick_budget: String,
}

impl EngineSettingsDraft {
    pub fn new(server: ServerSettings) -> Self {
        Self {
            locale: server.locale,
            display: super::display::logic::DisplayDraft::new(&server.display),
            tick_ms: server.tick_ms.to_string(),
            tick_budget: server.tick_budget.to_string(),
        }
    }

    pub fn parse_tick_ms(&self) -> Result<u32, ()> {
        let v: u32 = self.tick_ms.trim().parse().map_err(|_| ())?;
        if (MIN_TICK_MS..=MAX_TICK_MS).contains(&v) {
            Ok(v)
        } else {
            Err(())
        }
    }

    pub fn parse_budget(&self) -> Result<u32, ()> {
        let v: u32 = self.tick_budget.trim().parse().map_err(|_| ())?;
        if (MIN_TICK_BUDGET..=MAX_TICK_BUDGET).contains(&v) {
            Ok(v)
        } else {
            Err(())
        }
    }

    pub fn is_tick_valid(&self) -> bool {
        self.parse_tick_ms().is_ok()
    }

    pub fn is_budget_valid(&self) -> bool {
        self.parse_budget().is_ok()
    }

    pub fn is_valid(&self) -> bool {
        self.display.parse().is_some()
            && self.is_tick_valid()
            && self.is_budget_valid()
            && ["en", "uk"].contains(&self.locale.as_str())
    }

    pub fn is_dirty(&self, server: ServerSettings) -> bool {
        self.display != super::display::logic::DisplayDraft::new(&server.display)
            || self.locale != server.locale
            || self.tick_ms.trim() != server.tick_ms.to_string()
            || self.tick_budget.trim() != server.tick_budget.to_string()
    }

    pub fn can_save(&self, server: ServerSettings) -> bool {
        self.is_valid()
            && match (self.parse_tick_ms(), self.parse_budget()) {
                (Ok(tick), Ok(budget)) => {
                    self.display.parse().as_ref() != Some(&server.display)
                        || self.locale != server.locale
                        || tick != server.tick_ms
                        || budget != server.tick_budget
                }
                _ => false,
            }
    }

    pub fn to_server_settings(&self) -> Option<ServerSettings> {
        if !self.is_valid() {
            return None;
        }
        match (self.parse_tick_ms(), self.parse_budget()) {
            (Ok(tick_ms), Ok(tick_budget)) => Some(ServerSettings {
                tick_ms,
                tick_budget,
                locale: self.locale.clone(),
                display: self.display.parse()?,
            }),
            _ => None,
        }
    }
}
