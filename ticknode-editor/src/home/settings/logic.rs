use ticknode_shared::{ServerSettings, MAX_TICK_BUDGET, MAX_TICK_MS, MIN_TICK_BUDGET, MIN_TICK_MS};

/// Draft representation of server-wide engine settings edited in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettingsDraft {
    pub tick_ms: String,
    pub tick_budget: String,
}

impl EngineSettingsDraft {
    pub fn new(server: ServerSettings) -> Self {
        Self {
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
        self.is_tick_valid() && self.is_budget_valid()
    }

    pub fn is_dirty(&self, server: ServerSettings) -> bool {
        self.tick_ms.trim() != server.tick_ms.to_string()
            || self.tick_budget.trim() != server.tick_budget.to_string()
    }

    pub fn can_save(&self, server: ServerSettings) -> bool {
        self.is_valid()
            && match (self.parse_tick_ms(), self.parse_budget()) {
                (Ok(tick), Ok(budget)) => tick != server.tick_ms || budget != server.tick_budget,
                _ => false,
            }
    }

    pub fn to_server_settings(&self) -> Option<ServerSettings> {
        match (self.parse_tick_ms(), self.parse_budget()) {
            (Ok(tick_ms), Ok(tick_budget)) => Some(ServerSettings { tick_ms, tick_budget }),
            _ => None,
        }
    }
}
