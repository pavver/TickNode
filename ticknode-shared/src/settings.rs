use serde::{Deserialize, Serialize};

pub const MIN_TICK_MS: u32 = 1;
pub const MAX_TICK_MS: u32 = 10_000;
pub const DEFAULT_TICK_MS: u32 = 100;

pub const MIN_TICK_BUDGET: u32 = 0;
pub const MAX_TICK_BUDGET: u32 = 1_000_000;
pub const DEFAULT_TICK_BUDGET: u32 = 0;

/// Engine settings persisted on the server and shared with connected editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSettings {
    pub tick_ms: u32,
    pub tick_budget: u32, /* 0 = unlimited */
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            tick_ms: DEFAULT_TICK_MS,
            tick_budget: DEFAULT_TICK_BUDGET,
        }
    }
}

impl ServerSettings {
    pub fn validated(mut self) -> Self {
        self.tick_ms = self.tick_ms.clamp(MIN_TICK_MS, MAX_TICK_MS);
        self.tick_budget = self.tick_budget.clamp(MIN_TICK_BUDGET, MAX_TICK_BUDGET);
        self
    }
}

#[cfg(test)]
mod tests;
