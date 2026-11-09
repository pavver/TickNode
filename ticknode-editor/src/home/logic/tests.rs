use super::*;

#[test]
fn test_can_move_up() {
    assert!(!can_move_up(0));
    assert!(can_move_up(1));
    assert!(can_move_up(2));
}

#[test]
fn test_can_move_down() {
    assert!(!can_move_down(0, 0));
    assert!(!can_move_down(0, 1));
    assert!(can_move_down(0, 2));
    assert!(!can_move_down(1, 2));
    assert!(can_move_down(1, 3));
}

#[test]
fn test_sheet_last_run_age() {
    assert_eq!(sheet_last_run_age(None, 1000.0, 2000.0), None);
    assert_eq!(sheet_last_run_age(Some(5000), 1000.0, 1000.0), Some(5000.0));
    assert_eq!(sheet_last_run_age(Some(5000), 1000.0, 3500.0), Some(7500.0));
    // Clock jitter or clock step backwards clamps elapsed to 0
    assert_eq!(sheet_last_run_age(Some(5000), 1000.0, 500.0), Some(5000.0));
}

#[test]
fn test_pending_create_tracker() {
    let mut tracker = PendingCreateTracker::new();
    assert!(!tracker.is_pending());
    assert!(!tracker.on_created());

    tracker.request_create();
    assert!(tracker.is_pending());
    assert!(tracker.on_created());
    assert!(!tracker.is_pending());
    assert!(!tracker.on_created());

    tracker.request_create();
    tracker.clear();
    assert!(!tracker.is_pending());
    assert!(!tracker.on_created());
}
