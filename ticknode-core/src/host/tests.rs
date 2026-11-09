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
    assert_eq!(host.take_finished(), vec![(other, second, Ok(TaskOutput::Delay))]);
    assert!(host.take_finished().is_empty());
}

#[test]
fn core_c_zero_duration_tick_and_zero_delay_still_wait_one_tick() {
    let mut host = HostState { tick_duration: Duration::ZERO, current_tick: 10, ..Default::default() };
    assert_eq!(host.ticks_for(Duration::from_millis(7)), 7);
    let owner = NodeId::generate();
    let task = host.start_delay(owner, 0);
    assert!(host.take_finished().is_empty(), "a zero delay cannot complete in its starting tick");
    host.current_tick += 1;
    assert_eq!(host.take_finished(), vec![(owner, task, Ok(TaskOutput::Delay))]);
}
