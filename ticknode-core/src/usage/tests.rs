use std::time::Duration;

use super::*;

#[test]
fn test_ring_buffer_rollover_and_capping() {
    let mut tracker = UsageTracker::new(1000 * 3600);
    let node_lookup = |_id: NodeId| Some(("math/add".to_string(), None));

    // Record work for 100 different nodes
    for i in 0..100 {
        let node_id = NodeId::generate();
        tracker.record_node(node_id, (i + 1) as u32);
    }
    tracker.record_tick(Duration::from_micros(150), 5050, 2);

    // Step by 1 minute
    let completed_hour = tracker.step_time(
        1000 * 3600 + 60,
        &[(
            "math",
            PluginUsageMetrics {
                fuel: 1000,
                calls: 5,
                traps: 0,
            },
        )],
        node_lookup,
    );
    assert!(completed_hour.is_none());

    let report = tracker.report(10, &[], node_lookup);
    assert_eq!(report.minutes.len(), 2); // 1 completed + 1 active snapshot
    let prev_minute = &report.minutes[0];
    assert_eq!(prev_minute.ticks, 1);
    assert_eq!(prev_minute.avg_tick_us, 150);
    assert_eq!(prev_minute.max_tick_us, 150);
    assert_eq!(prev_minute.total_spent, 5050);
    assert_eq!(prev_minute.deferred_activations, 2);
    assert_eq!(prev_minute.top_nodes.len(), TRACKED_TOP_NODES);
    assert!(prev_minute.other_node_cost > 0);
    assert!(prev_minute.other_node_executions > 0);
    assert_eq!(prev_minute.plugins.len(), 1);
    assert_eq!(prev_minute.plugins[0].fuel, 1000);

    // Advance 65 minutes to verify rollover of 60 minute buckets
    for m in 2..=65 {
        let ts = 1000 * 3600 + m * 60;
        tracker.record_tick(Duration::from_micros(100), 10, 0);
        let completed = tracker.step_time(ts, &[], node_lookup);
        if m == 60 {
            assert!(completed.is_some());
            let hr = completed.unwrap();
            assert_eq!(hr.ticks, 60);
            assert_eq!(hr.timestamp, 1000 * 3600);
        }
    }

    let report = tracker.report(100, &[], node_lookup);
    assert_eq!(tracker.minute_buckets.len(), MINUTE_BUCKETS_COUNT);
    assert_eq!(report.minutes.len(), MINUTE_BUCKETS_COUNT + 1); // 60 + 1 active
}

#[test]
fn test_idle_bucket_detection() {
    let bucket = UsageBucket::default();
    assert!(bucket.is_idle());

    let active_bucket = UsageBucket {
        total_spent: 10,
        ..Default::default()
    };
    assert!(!active_bucket.is_idle());
}

#[test]
fn test_hour_rollover_and_load_hour_buckets() {
    let mut tracker = UsageTracker::new(0);
    let node_lookup = |_id: NodeId| Some(("console".to_string(), None));

    let mut preloaded = Vec::new();
    for h in 0..50 {
        preloaded.push(UsageBucket {
            timestamp: h * 3600,
            ticks: 100,
            avg_tick_us: 50,
            max_tick_us: 100,
            total_spent: 500,
            ..Default::default()
        });
    }
    tracker.load_hour_buckets(preloaded);
    assert_eq!(tracker.hour_buckets.len(), HOUR_BUCKETS_COUNT);

    let report = tracker.report(1, &[], node_lookup);
    assert_eq!(report.hours.len(), HOUR_BUCKETS_COUNT + 1); // 48 + 1 active
}

#[test]
fn test_engine_usage_integration() {
    let mut engine = crate::TickEngine::new();
    let report = engine.usage_report(100);
    assert!(!report.minutes.is_empty());
    let step = engine.step_usage(100);
    assert!(step.is_none());
    let final_hr = engine.finalize_usage_hour(100);
    assert_eq!(final_hr.timestamp, 0);
}
