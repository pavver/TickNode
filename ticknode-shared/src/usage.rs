use serde::{Deserialize, Serialize};

use crate::{NodeId, SheetId};

/// Maximum number of top nodes tracked individually per time window.
pub const TRACKED_TOP_NODES: usize = 64;
/// Number of 1-minute buckets in the rolling window (1 hour of minute history).
pub const MINUTE_BUCKETS_COUNT: usize = 60;
/// Number of 1-hour buckets in the rolling window (48 hours of hour history).
pub const HOUR_BUCKETS_COUNT: usize = 48;

/// WASM fuel and call accounting for a single plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PluginUsage {
    pub plugin: String,
    pub fuel: u64,
    pub calls: u64,
    pub traps: u32,
}

/// Node execution and spent cost accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeUsage {
    pub node_id: NodeId,
    pub type_name: String,
    pub sheet_id: Option<SheetId>,
    pub executions: u64,
    pub cost: u64,
}

/// Work performed aggregated by scenario sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetUsage {
    pub sheet_id: Option<SheetId>,
    pub cost: u64,
    pub executions: u64,
}

/// Aggregated usage statistics within a single time bucket (1 minute or 1 hour).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageBucket {
    /// Window start timestamp as Unix seconds.
    pub timestamp: i64,
    /// Number of engine ticks executed in this window.
    pub ticks: u32,
    /// Average tick duration in microseconds.
    pub avg_tick_us: u32,
    /// Maximum tick duration in microseconds.
    pub max_tick_us: u32,
    /// Total cost units spent according to budget accounting.
    pub total_spent: u64,
    /// Number of queue activations deferred due to work budget limits.
    pub deferred_activations: u64,
    /// Plugin fuel and call stats.
    pub plugins: Vec<PluginUsage>,
    /// Top nodes sorted by spent cost descending (at most `TRACKED_TOP_NODES`).
    pub top_nodes: Vec<NodeUsage>,
    /// Cost spent by nodes outside the top list.
    pub other_node_cost: u64,
    /// Executions by nodes outside the top list.
    pub other_node_executions: u64,
    /// Aggregated cost and executions per sheet.
    pub sheets: Vec<SheetUsage>,
}

impl UsageBucket {
    /// Whether no automations or plugins ran during this bucket.
    pub fn is_idle(&self) -> bool {
        self.total_spent == 0
            && self.top_nodes.is_empty()
            && self.other_node_cost == 0
            && self.deferred_activations == 0
            && self.plugins.iter().all(|p| p.fuel == 0 && p.calls == 0)
    }
}

/// Complete usage report delivered to the editor upon request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageReport {
    /// Rolling 1-minute buckets (oldest first).
    pub minutes: Vec<UsageBucket>,
    /// Rolling 1-hour buckets (oldest first).
    pub hours: Vec<UsageBucket>,
    /// Current engine tick number.
    pub current_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_bucket_is_idle() {
        let mut b = UsageBucket::default();
        assert!(b.is_idle());

        b.plugins.push(PluginUsage::default());
        assert!(b.is_idle());

        let mut b1 = b.clone();
        b1.total_spent = 1;
        assert!(!b1.is_idle());

        let mut b2 = b.clone();
        b2.other_node_cost = 1;
        assert!(!b2.is_idle());

        let mut b3 = b.clone();
        b3.deferred_activations = 1;
        assert!(!b3.is_idle());

        let mut b4 = b.clone();
        b4.top_nodes.push(NodeUsage {
            node_id: NodeId::generate(),
            type_name: "test".into(),
            sheet_id: None,
            executions: 1,
            cost: 1,
        });
        assert!(!b4.is_idle());

        let mut b5 = b.clone();
        b5.plugins[0].fuel = 100;
        assert!(!b5.is_idle());

        let mut b6 = b.clone();
        b6.plugins[0].calls = 1;
        assert!(!b6.is_idle());
    }
}
