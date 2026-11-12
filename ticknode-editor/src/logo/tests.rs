use super::*;
use ticknode_shared::{SheetId, SheetInfo};

fn sheet(last_executed_ms: Option<u64>) -> SheetSummary {
    SheetSummary {
        info: SheetInfo::new(SheetId::generate(), "s"),
        nodes: 1,
        errors: 0,
        last_executed_ms,
    }
}

#[test]
fn animates_while_connecting_even_without_activity() {
    assert!(logo_active(
        ConnStatus::Connecting,
        f64::NEG_INFINITY,
        10_000.0
    ));
    assert!(!logo_active(
        ConnStatus::Online,
        f64::NEG_INFINITY,
        10_000.0
    ));
    assert!(!logo_active(
        ConnStatus::Offline,
        f64::NEG_INFINITY,
        10_000.0
    ));
}

#[test]
fn animates_for_one_cycle_after_activity() {
    assert!(logo_active(ConnStatus::Online, 10_000.0, 13_999.0));
    assert!(!logo_active(ConnStatus::Online, 10_000.0, 14_000.0));
}

#[test]
fn summaries_report_the_freshest_recent_run() {
    let sheets = [sheet(Some(3000)), sheet(None), sheet(Some(500))];
    assert_eq!(summary_activity(&sheets, 10_000.0), Some(9_500.0));
}

#[test]
fn old_or_missing_runs_are_not_activity() {
    assert_eq!(summary_activity(&[sheet(Some(60_000))], 10_000.0), None);
    assert_eq!(summary_activity(&[sheet(None)], 10_000.0), None);
    assert_eq!(summary_activity(&[], 10_000.0), None);
}
