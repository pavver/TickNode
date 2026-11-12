/// Per-tick scheduling policy. Unlimited by default; independent of graph execution.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TickBudget {
    pub limit: Option<u32>,
}

impl TickBudget {
    pub(crate) fn should_defer(&self, spent: u32, cost: u32) -> bool {
        self.limit
            .is_some_and(|limit| spent > 0 && u64::from(spent) + u64::from(cost) > u64::from(limit))
    }
}

/// Work performed and queue activations postponed during the last tick.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TickStats {
    pub spent: u32,
    pub deferred: usize,
}

impl TickStats {
    pub(crate) fn charge(&mut self, cost: u32) {
        self.spent = self.spent.saturating_add(cost);
    }
}
