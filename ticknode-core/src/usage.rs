use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use ticknode_shared::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PluginUsageMetrics {
    pub fuel: u64,
    pub calls: u64,
    pub traps: u32,
}

#[derive(Debug, Default, Clone)]
struct BucketAccumulator {
    ticks: u32,
    total_tick_time_us: u64,
    max_tick_us: u32,
    total_spent: u64,
    deferred_activations: u64,
    node_work: HashMap<NodeId, (u64, u64)>,
    other_node_cost: u64,
    other_node_executions: u64,
    sheet_work: HashMap<Option<SheetId>, (u64, u64)>,
    plugin_fuel: HashMap<String, (u64, u64, u32)>,
}

impl BucketAccumulator {
    fn record_tick(&mut self, wall_time: Duration, spent: u32, deferred: usize) {
        self.ticks = self.ticks.saturating_add(1);
        let us = wall_time.as_micros() as u64;
        self.total_tick_time_us = self.total_tick_time_us.saturating_add(us);
        let us_32 = (us.min(u32::MAX as u64)) as u32;
        if us_32 > self.max_tick_us {
            self.max_tick_us = us_32;
        }
        self.total_spent = self.total_spent.saturating_add(spent as u64);
        self.deferred_activations = self.deferred_activations.saturating_add(deferred as u64);
    }

    fn record_node(&mut self, node_id: NodeId, cost: u32) {
        if let Some(entry) = self.node_work.get_mut(&node_id) {
            entry.0 = entry.0.saturating_add(1);
            entry.1 = entry.1.saturating_add(cost as u64);
        } else if self.node_work.len() < 128 {
            self.node_work.insert(node_id, (1, cost as u64));
        } else {
            self.other_node_executions = self.other_node_executions.saturating_add(1);
            self.other_node_cost = self.other_node_cost.saturating_add(cost as u64);
        }
    }

    fn record_sheet(&mut self, sheet_id: Option<SheetId>, cost: u32) {
        let entry = self.sheet_work.entry(sheet_id).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = entry.1.saturating_add(cost as u64);
    }

    fn add_plugins(&mut self, plugins: &[(&str, PluginUsageMetrics)]) {
        for (name, metrics) in plugins {
            let entry = self
                .plugin_fuel
                .entry((*name).to_string())
                .or_insert((0, 0, 0));
            entry.0 = entry.0.saturating_add(metrics.fuel);
            entry.1 = entry.1.saturating_add(metrics.calls);
            entry.2 = entry.2.saturating_add(metrics.traps);
        }
    }

    fn merge_bucket(&mut self, bucket: &UsageBucket) {
        self.ticks = self.ticks.saturating_add(bucket.ticks);
        let bucket_total_us = (bucket.avg_tick_us as u64).saturating_mul(bucket.ticks as u64);
        self.total_tick_time_us = self.total_tick_time_us.saturating_add(bucket_total_us);
        if bucket.max_tick_us > self.max_tick_us {
            self.max_tick_us = bucket.max_tick_us;
        }
        self.total_spent = self.total_spent.saturating_add(bucket.total_spent);
        self.deferred_activations = self
            .deferred_activations
            .saturating_add(bucket.deferred_activations);

        for node in &bucket.top_nodes {
            if let Some(entry) = self.node_work.get_mut(&node.node_id) {
                entry.0 = entry.0.saturating_add(node.executions);
                entry.1 = entry.1.saturating_add(node.cost);
            } else if self.node_work.len() < 128 {
                self.node_work
                    .insert(node.node_id, (node.executions, node.cost));
            } else {
                self.other_node_executions =
                    self.other_node_executions.saturating_add(node.executions);
                self.other_node_cost = self.other_node_cost.saturating_add(node.cost);
            }
        }
        self.other_node_cost = self.other_node_cost.saturating_add(bucket.other_node_cost);
        self.other_node_executions = self
            .other_node_executions
            .saturating_add(bucket.other_node_executions);

        for sheet in &bucket.sheets {
            let entry = self.sheet_work.entry(sheet.sheet_id).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(sheet.executions);
            entry.1 = entry.1.saturating_add(sheet.cost);
        }

        for plugin in &bucket.plugins {
            let entry = self
                .plugin_fuel
                .entry(plugin.plugin.clone())
                .or_insert((0, 0, 0));
            entry.0 = entry.0.saturating_add(plugin.fuel);
            entry.1 = entry.1.saturating_add(plugin.calls);
            entry.2 = entry.2.saturating_add(plugin.traps);
        }
    }

    fn finalize(
        self,
        timestamp: i64,
        node_lookup: &impl Fn(NodeId) -> Option<(String, Option<SheetId>)>,
    ) -> UsageBucket {
        let avg_tick_us = if self.ticks > 0 {
            (self.total_tick_time_us / (self.ticks as u64)).min(u32::MAX as u64) as u32
        } else {
            0
        };

        let mut nodes: Vec<(NodeId, (u64, u64))> = self.node_work.into_iter().collect();
        nodes.sort_by(|a, b| b.1.1.cmp(&a.1.1).then_with(|| b.1.0.cmp(&a.1.0)));

        let mut top_nodes = Vec::new();
        let mut other_cost = self.other_node_cost;
        let mut other_executions = self.other_node_executions;

        for (i, (id, (execs, cost))) in nodes.into_iter().enumerate() {
            if i < TRACKED_TOP_NODES {
                let (type_name, sheet_id) =
                    node_lookup(id).unwrap_or_else(|| ("unknown".into(), None));
                top_nodes.push(NodeUsage {
                    node_id: id,
                    type_name,
                    sheet_id,
                    executions: execs,
                    cost,
                });
            } else {
                other_cost = other_cost.saturating_add(cost);
                other_executions = other_executions.saturating_add(execs);
            }
        }

        let mut sheets: Vec<SheetUsage> = self
            .sheet_work
            .into_iter()
            .map(|(sheet_id, (executions, cost))| SheetUsage {
                sheet_id,
                cost,
                executions,
            })
            .collect();
        sheets.sort_by_key(|a| std::cmp::Reverse(a.cost));

        let mut plugins: Vec<PluginUsage> = self
            .plugin_fuel
            .into_iter()
            .map(|(plugin, (fuel, calls, traps))| PluginUsage {
                plugin,
                fuel,
                calls,
                traps,
            })
            .collect();
        plugins.sort_by_key(|a| std::cmp::Reverse(a.fuel));

        UsageBucket {
            timestamp,
            ticks: self.ticks,
            avg_tick_us,
            max_tick_us: self.max_tick_us,
            total_spent: self.total_spent,
            deferred_activations: self.deferred_activations,
            plugins,
            top_nodes,
            other_node_cost: other_cost,
            other_node_executions: other_executions,
            sheets,
        }
    }
}

/// Rolling window aggregator for usage and load statistics.
#[derive(Debug)]
pub struct UsageTracker {
    current_minute: BucketAccumulator,
    minute_buckets: VecDeque<UsageBucket>,
    current_hour: BucketAccumulator,
    hour_buckets: VecDeque<UsageBucket>,
    current_minute_ts: i64,
    current_hour_ts: i64,
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new(0)
    }
}

impl UsageTracker {
    pub fn new(initial_ts: i64) -> Self {
        let minute_ts = if initial_ts > 0 {
            (initial_ts / 60) * 60
        } else {
            0
        };
        let hour_ts = if initial_ts > 0 {
            (initial_ts / 3600) * 3600
        } else {
            0
        };
        Self {
            current_minute: BucketAccumulator::default(),
            minute_buckets: VecDeque::with_capacity(MINUTE_BUCKETS_COUNT),
            current_hour: BucketAccumulator::default(),
            hour_buckets: VecDeque::with_capacity(HOUR_BUCKETS_COUNT),
            current_minute_ts: minute_ts,
            current_hour_ts: hour_ts,
        }
    }

    pub fn record_tick(&mut self, wall_time: Duration, spent: u32, deferred: usize) {
        self.current_minute.record_tick(wall_time, spent, deferred);
    }

    pub fn record_node(&mut self, node_id: NodeId, cost: u32) {
        self.current_minute.record_node(node_id, cost);
    }

    pub fn record_sheet(&mut self, sheet_id: Option<SheetId>, cost: u32) {
        self.current_minute.record_sheet(sheet_id, cost);
    }

    pub fn set_hour_history(&mut self, buckets: Vec<UsageBucket>) {
        self.hour_buckets.clear();
        for b in buckets.into_iter().take(HOUR_BUCKETS_COUNT) {
            self.hour_buckets.push_back(b);
        }
    }

    /// Advances time and rolls over completed minute and hour buckets.
    /// Returns `Some(UsageBucket)` whenever an hour bucket is completed.
    pub fn step_time(
        &mut self,
        now_ts: i64,
        plugins: &[(&str, PluginUsageMetrics)],
        node_lookup: impl Fn(NodeId) -> Option<(String, Option<SheetId>)>,
    ) -> Option<UsageBucket> {
        let target_minute_ts = (now_ts / 60) * 60;
        let target_hour_ts = (now_ts / 3600) * 3600;

        if self.current_minute_ts == 0 && target_minute_ts > 0 {
            self.current_minute_ts = target_minute_ts;
            self.current_hour_ts = target_hour_ts;
            return None;
        }

        let mut completed_hour = None;

        if target_minute_ts > self.current_minute_ts {
            self.current_minute.add_plugins(plugins);
            let minute_acc = std::mem::take(&mut self.current_minute);
            let minute_bucket = minute_acc.finalize(self.current_minute_ts, &node_lookup);
            self.current_hour.merge_bucket(&minute_bucket);

            if self.minute_buckets.len() >= MINUTE_BUCKETS_COUNT {
                self.minute_buckets.pop_front();
            }
            self.minute_buckets.push_back(minute_bucket);
            self.current_minute_ts = target_minute_ts;

            if target_hour_ts > self.current_hour_ts {
                let hour_acc = std::mem::take(&mut self.current_hour);
                let hour_bucket = hour_acc.finalize(self.current_hour_ts, &node_lookup);

                if self.hour_buckets.len() >= HOUR_BUCKETS_COUNT {
                    self.hour_buckets.pop_front();
                }
                self.hour_buckets.push_back(hour_bucket.clone());
                self.current_hour_ts = target_hour_ts;
                completed_hour = Some(hour_bucket);
            }
        }

        completed_hour
    }

    /// Finalizes the currently active hour (e.g. during engine shutdown).
    pub fn finalize_current_hour(
        &mut self,
        plugins: &[(&str, PluginUsageMetrics)],
        node_lookup: impl Fn(NodeId) -> Option<(String, Option<SheetId>)>,
    ) -> UsageBucket {
        self.current_minute.add_plugins(plugins);
        let minute_acc = std::mem::take(&mut self.current_minute);
        let minute_bucket = minute_acc.finalize(self.current_minute_ts, &node_lookup);
        self.current_hour.merge_bucket(&minute_bucket);
        let hour_acc = std::mem::take(&mut self.current_hour);
        hour_acc.finalize(self.current_hour_ts, &node_lookup)
    }

    /// Preloads historical hour buckets (e.g. read from JSONL storage on startup).
    pub fn load_hour_buckets(&mut self, buckets: Vec<UsageBucket>) {
        for b in buckets {
            if self.hour_buckets.len() >= HOUR_BUCKETS_COUNT {
                self.hour_buckets.pop_front();
            }
            self.hour_buckets.push_back(b);
        }
    }

    /// Builds a usage report for the editor, including snapshots of the active windows.
    pub fn report(
        &self,
        current_tick: u64,
        plugins: &[(&str, PluginUsageMetrics)],
        node_lookup: impl Fn(NodeId) -> Option<(String, Option<SheetId>)>,
    ) -> UsageReport {
        let mut minutes: Vec<UsageBucket> = self.minute_buckets.iter().cloned().collect();
        let mut active_min = self.current_minute.clone();
        active_min.add_plugins(plugins);
        let active_min_bucket = active_min.finalize(self.current_minute_ts, &node_lookup);
        minutes.push(active_min_bucket.clone());

        let mut hours: Vec<UsageBucket> = self.hour_buckets.iter().cloned().collect();
        let mut active_hr = self.current_hour.clone();
        active_hr.merge_bucket(&active_min_bucket);
        hours.push(active_hr.finalize(self.current_hour_ts, &node_lookup));

        UsageReport {
            minutes,
            hours,
            current_tick,
        }
    }
}
