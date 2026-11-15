use chrono::Utc;
use tracing::info;

impl super::Actor {
    pub(super) fn step_usage(&mut self) {
        let now_ts = Utc::now().timestamp();
        if let Some(bucket) = self.engine.step_usage(now_ts) {
            let total_fuel: u64 = bucket.plugins.iter().map(|p| p.fuel).sum();
            info!(
                timestamp = bucket.timestamp,
                ticks = bucket.ticks,
                avg_tick_us = bucket.avg_tick_us,
                max_tick_us = bucket.max_tick_us,
                total_fuel,
                total_work = bucket.total_spent,
                "Usage statistics hour summary"
            );
            if self.server_settings.usage_stats
                && !bucket.is_idle()
                && let Err(e) = self.storage.append_hour_bucket(&bucket)
            {
                tracing::warn!("Failed to persist usage hour bucket: {e}");
            }
        }
    }

    pub(super) fn save_usage_stats_on_shutdown(&mut self) {
        if !self.server_settings.usage_stats {
            return;
        }
        let now_ts = Utc::now().timestamp();
        let bucket = self.engine.finalize_usage_hour(now_ts);
        if !bucket.is_idle()
            && let Err(e) = self.storage.append_hour_bucket(&bucket)
        {
            tracing::warn!("Failed to persist usage hour bucket on shutdown: {e}");
        }
    }
}
