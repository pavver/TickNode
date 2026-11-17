use super::*;

#[test]
fn control_states_select_the_styles_defined_by_the_design_system() {
    let styles = [
        include_str!("../../styles/base.css"),
        include_str!("../../styles/panels.css"),
    ]
    .concat();
    for (state, class) in [
        (ControlState::Idle, "st-idle"),
        (ControlState::Pending, "st-pending"),
        (ControlState::Success, "st-success"),
        (ControlState::Error, "st-error"),
    ] {
        assert_eq!(state.class(), class);
        assert!(styles.contains(&format!(".{class}")));
    }
}

#[test]
fn idle_explicitly_overrides_the_operator_controls_local_result() {
    assert_eq!(
        to_op(ControlState::Idle),
        operator_ui::prelude::ControlState::Idle
    );
}

#[test]
fn terminal_entries_dim_the_time_and_reset_each_log_kind() {
    use crate::state::{LogKind, LogLine};

    for (kind, color) in [
        (LogKind::Node, ""),
        (LogKind::Error, "\x1b[91m"),
        (LogKind::System, "\x1b[96m"),
    ] {
        let line = LogLine {
            id: 1,
            time: "12:34:56".into(),
            kind,
            text: "Example log".into(),
        };
        assert_eq!(
            log_text(&line),
            format!("\x1b[2m12:34:56\x1b[22m {color}Example log\x1b[0m")
        );
    }
}

#[test]
fn async_switch_waits_for_confirmation_and_reports_errors_without_changing_the_value() {
    use any_spawner::Executor;
    use futures::FutureExt;
    use operator_ui::prelude::ActionResult;

    let _ = Executor::init_futures_executor();
    let owner = Owner::new();
    owner.with(|| {
        let checked = RwSignal::new(false);
        let state = RwSignal::new(ControlState::Idle);
        let requested = RwSignal::new(None);
        let toggle = confirmed_toggle(
            checked.into(),
            state,
            Callback::new(move |value| requested.set(Some(value))),
        );
        let mut response = toggle.call(true);
        assert_eq!(requested.get_untracked(), Some(true));
        assert_eq!(state.get_untracked(), ControlState::Pending);
        assert!(!checked.get_untracked());
        assert!((&mut response).now_or_never().is_none());

        checked.set(true);
        Executor::poll_local();
        assert!((&mut response).now_or_never().is_none());
        state.set(ControlState::Idle);
        Executor::poll_local();
        assert_eq!(response.now_or_never(), Some(ActionResult::Success));

        let response = toggle.call(false);
        state.set(ControlState::Error);
        Executor::poll_local();
        assert_eq!(response.now_or_never(), Some(ActionResult::Error(None)));
        assert!(checked.get_untracked());
    });
}
