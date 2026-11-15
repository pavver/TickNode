use super::*;
use std::time::Duration;

#[test]
fn test_get_usage_command_replies_with_report() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());

    let (reply, mut rx) = mpsc::unbounded_channel();
    let (events, created) = actor.apply(ClientMessage::GetUsage, None, &reply).unwrap();

    assert!(events.is_empty());
    assert!(created.is_none());

    let response = rx.try_recv().unwrap();
    match response {
        ServerMessage::Usage(report) => {
            assert_eq!(report.current_tick, 0);
            assert_eq!(report.minutes.len(), 1);
            assert_eq!(report.hours.len(), 1);
        }
        other => panic!("expected ServerMessage::Usage, got {other:?}"),
    }
}

#[test]
fn test_step_usage_throttling_and_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    actor.server_settings.usage_stats = true;

    // Simulate some engine work so bucket is not idle
    actor
        .engine
        .usage
        .record_tick(Duration::from_millis(5), 10, 0);

    // Initial step at t = 1000
    actor.engine.step_usage(1000);

    // An hour later, cross boundary
    actor.step_usage();

    // Verify shutdown finalize saves if not idle
    actor
        .engine
        .usage
        .record_tick(Duration::from_millis(2), 5, 0);
    actor.save_usage_stats_on_shutdown();

    let loaded = actor.storage.load_recent_usage_hours(10).unwrap();
    assert!(!loaded.is_empty());
    assert!(loaded[0].ticks > 0);
}

#[test]
fn test_step_usage_disabled_does_not_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    actor.server_settings.usage_stats = false;

    actor
        .engine
        .usage
        .record_tick(Duration::from_millis(5), 10, 0);
    actor.save_usage_stats_on_shutdown();

    let loaded = actor.storage.load_recent_usage_hours(10).unwrap();
    assert!(loaded.is_empty());
}
