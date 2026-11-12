use super::*;

#[test]
fn core_c_task_handles_cancel_only_the_owner_and_complete_in_start_order() {
    let mut host = HostState::default();
    let owner = NodeId::generate();
    let other = NodeId::generate();
    let first = host.start_delay(owner, 2);
    let second = host.start_delay(other, 2);
    // Task zero is a valid handle, not a sentinel for an absent task.
    assert_eq!(first, 0);
    assert_ne!(first, second);
    assert!(!host.cancel_task(other, first));
    assert!(host.cancel_task(owner, first));
    assert!(!host.cancel_task(owner, first));
    host.current_tick = 2;
    assert_eq!(
        host.take_finished(),
        vec![(other, second, Ok(TaskOutput::Delay))]
    );
    assert!(host.take_finished().is_empty());
}

#[test]
fn core_c_zero_duration_tick_and_zero_delay_still_wait_one_tick() {
    let mut host = HostState {
        tick_duration: Duration::ZERO,
        current_tick: 10,
        ..Default::default()
    };
    assert_eq!(host.ticks_for(Duration::from_millis(7)), 7);
    let owner = NodeId::generate();
    let task = host.start_delay(owner, 0);
    assert!(
        host.take_finished().is_empty(),
        "a zero delay cannot complete in its starting tick"
    );
    host.current_tick += 1;
    assert_eq!(
        host.take_finished(),
        vec![(owner, task, Ok(TaskOutput::Delay))]
    );
}

#[test]
fn duration_values_round_up_without_losing_submillisecond_time() {
    let host = HostState::default();
    for (value, expected) in [
        (DataValue::Ticks(0), Some(0)),
        (DataValue::Ticks(u64::MAX), Some(u64::MAX)),
        (DataValue::Duration(Duration::ZERO), Some(0)),
        (DataValue::Duration(Duration::from_nanos(1)), Some(1)),
        (
            DataValue::Duration(Duration::from_millis(100) + Duration::from_nanos(1)),
            Some(2),
        ),
        (DataValue::Duration(Duration::from_millis(250)), Some(3)),
        (DataValue::from(10i64), None),
    ] {
        assert_eq!(host.ticks_for_value(&value), expected);
    }
}

#[test]
fn timers_track_first_started_task_count_cancellation_and_delivery() {
    let mut engine = crate::TickEngine::new();
    let owner = NodeId::generate();
    let other = NodeId::generate();
    let host = &mut engine.host_state;
    host.current_tick = 10;
    let first = host.start_delay(owner, 10);
    let second = host.start_delay(owner, 2); // Earlier deadline still does not take over the bar.
    host.start_delay(other, 4);
    let timer = |engine: &crate::TickEngine, node| {
        engine
            .delay_timers()
            .into_iter()
            .find(|t| t.node == node)
            .unwrap()
    };
    assert_eq!(
        timer(&engine, owner),
        NodeTimer {
            node: owner,
            total_ms: 1000,
            remaining_ms: 1000,
            count: 2
        }
    );
    assert_eq!(
        engine
            .host_state
            .delay_timers_with_tasks()
            .into_iter()
            .find(|(_, t)| t.node == owner)
            .unwrap()
            .0,
        first
    );
    engine.host_state.current_tick = 11;
    assert_eq!(timer(&engine, owner).remaining_ms, 900);
    engine.host_state.current_tick = 12;
    assert_eq!(
        engine.host_state.take_finished(),
        vec![(owner, second, Ok(TaskOutput::Delay))]
    );
    assert_eq!(timer(&engine, owner).count, 1);
    // Ignore-style retrigger leaves the active task unchanged.
    assert_eq!(timer(&engine, owner).total_ms, 1000);
    // Reset-style retrigger cancels the old timer and starts a new one.
    assert!(engine.host_state.cancel_task(owner, first));
    let reset = engine.host_state.start_delay(owner, 5);
    assert_eq!(
        timer(&engine, owner),
        NodeTimer {
            node: owner,
            total_ms: 500,
            remaining_ms: 500,
            count: 1
        }
    );
    engine.host_state.tick_duration = Duration::from_millis(20);
    assert_eq!(timer(&engine, owner).total_ms, 100);
    engine.host_state.current_tick = 17;
    assert_eq!(timer(&engine, owner).remaining_ms, 0);
    assert!(
        engine
            .host_state
            .take_finished()
            .iter()
            .any(|(_, id, _)| *id == reset)
    );
    assert!(engine.delay_timers().is_empty());
    engine.host_state.start_delay(owner, 0);
    assert_eq!(timer(&engine, owner).total_ms, 20);
    engine.remove_node(owner);
    assert!(engine.delay_timers().is_empty());
}
