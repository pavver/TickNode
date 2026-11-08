use super::*;

#[test]
fn task_completion_always_emits_and_only_clears_the_matching_active_task() {
    for (kind, base) in [
        ("delay/queue", 0),
        ("delay/ignore", 10),
        ("delay/reset", 20),
    ] {
        let mut node = DelayPlugin::create(kind, NodeId::generate()).unwrap();
        assert_eq!(node.definition().type_name, kind);
        assert_eq!(
            node.execute(
                Trigger::Task {
                    id: 8,
                    result: Err("cancelled".into())
                },
                &Inputs::default()
            ),
            vec![Output::Exec(PinId(base + 1))]
        );
        assert!(
            node.execute(Trigger::Pin(PinId(base + 9)), &Inputs::default())
                .is_empty()
        );
    }
    let mut node = DelayNode {
        mode: Mode::Ignore,
        active: Some(9),
    };
    assert!(
        node.execute(Trigger::Pin(PinId(10)), &Inputs::default())
            .is_empty()
    );
    assert_eq!(node.active, Some(9));
    for (id, expected) in [(8, Some(9)), (9, None)] {
        assert_eq!(
            node.execute(
                Trigger::Task {
                    id,
                    result: Ok(TaskOutput::Delay)
                },
                &Inputs::default()
            ),
            vec![Output::Exec(PinId(11))]
        );
        assert_eq!(node.active, expected);
    }
    assert!(DelayPlugin::create("unknown", NodeId::generate()).is_none());
}

#[derive(Default)]
struct Clock {
    calls: Vec<String>,
    fail: bool,
}
impl Scheduler for Clock {
    fn delay_ms(&mut self, ms: u64) -> Result<TaskId, String> {
        self.calls.push(format!("delay {ms}"));
        if self.fail {
            Err("failed".into())
        } else {
            Ok(42)
        }
    }
    fn cancel(&mut self, task: TaskId) {
        self.calls.push(format!("cancel {task}"));
    }
    fn error(&mut self, error: &str) {
        self.calls.push(format!("error {error}"));
    }
}
#[test]
fn scheduling_preserves_retrigger_modes_cancellation_and_failure() {
    for (mode, base, expected) in [
        (Mode::Queue, 0, vec!["delay 12"]),
        (Mode::Ignore, 10, vec![]),
        (Mode::Reset, 20, vec!["cancel 7", "delay 12"]),
    ] {
        let mut node = DelayNode {
            mode,
            active: Some(7),
        };
        let mut clock = Clock::default();
        let inputs = [(PinId(base + 2), 12.9.into())].into_iter().collect();
        assert!(
            node.execute_with(Trigger::Pin(PinId(base)), &inputs, &mut clock)
                .is_empty()
        );
        assert_eq!(clock.calls, expected);
        assert_eq!(
            node.active,
            if mode == Mode::Ignore {
                Some(7)
            } else {
                Some(42)
            }
        );
    }
    let mut node = DelayNode {
        mode: Mode::Reset,
        active: Some(7),
    };
    let mut clock = Clock {
        fail: true,
        ..Clock::default()
    };
    let inputs = [(PinId(22), (-1.0).into())].into_iter().collect();
    assert!(
        node.execute_with(Trigger::Pin(PinId(20)), &inputs, &mut clock)
            .is_empty()
    );
    assert_eq!(clock.calls, ["cancel 7", "delay 0", "error failed"]);
    assert_eq!(node.active, None);
}
