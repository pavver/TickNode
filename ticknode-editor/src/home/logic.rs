//! Pure logic helpers for the home page and scenario sheets.

/// Returns true if a sheet at `index` can be moved up (i.e. not the first item).
pub fn can_move_up(index: usize) -> bool {
    index > 0
}

/// Returns true if a sheet at `index` can be moved down (i.e. not the last item).
pub fn can_move_down(index: usize, total: usize) -> bool {
    total > 0 && index + 1 < total
}

/// Computes the live age in milliseconds for a sheet execution.
/// `last_executed_ms` was the age in ms at `received_at_ms`.
pub fn sheet_last_run_age(
    last_executed_ms: Option<u64>,
    received_at_ms: f64,
    now_ms: f64,
) -> Option<f64> {
    last_executed_ms.map(|ago| {
        let elapsed = (now_ms - received_at_ms).max(0.0);
        ago as f64 + elapsed
    })
}

/// Bookkeeping for sheet creation requests initiated by this client,
/// ensuring that only explicit "New sheet" creations navigate to the new sheet,
/// while duplicates or external creations stay on the home page.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PendingCreateTracker {
    pending: bool,
}

impl PendingCreateTracker {
    pub fn new() -> Self {
        Self { pending: false }
    }

    pub fn request_create(&mut self) {
        self.pending = true;
    }

    pub fn on_created(&mut self) -> bool {
        let was = self.pending;
        self.pending = false;
        was
    }

    pub fn clear(&mut self) {
        self.pending = false;
    }

    #[cfg(test)]
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests;
