//! Adapters for server-controlled Operator UI components.

use futures::channel::oneshot;
use leptos::prelude::*;
use operator_ui::prelude::{ActionResult, AsyncAction, ControlState as OpState};

use super::ControlState;
use crate::state::{LogKind, LogLine};

/// Include Idle explicitly so a completed local action cannot override the server's state.
pub fn to_op(state: ControlState) -> OpState {
    match state {
        ControlState::Idle => OpState::Idle,
        ControlState::Pending => OpState::Pending,
        ControlState::Success => OpState::Success,
        ControlState::Error => OpState::Error,
    }
}

/// Finish the switch's action only when its confirmed value or error arrives.
pub fn confirmed_toggle(
    checked: Signal<bool>,
    state: RwSignal<ControlState>,
    send: Callback<bool>,
) -> AsyncAction<bool> {
    AsyncAction::new(move |requested| {
        state.set(ControlState::Pending);
        let (reply, response) = oneshot::channel();
        let mut reply = Some(reply);
        let watch = RenderEffect::new(move |_| {
            let value = checked.get();
            let status = state.get();
            if status != ControlState::Pending
                && let Some(reply) = reply.take()
            {
                let result = if value == requested && status != ControlState::Error {
                    ActionResult::Success
                } else {
                    ActionResult::Error(None)
                };
                let _ = reply.send(result);
            }
        });
        send.run(requested);
        async move {
            let result = response.await.unwrap_or(ActionResult::Error(None));
            drop(watch);
            result
        }
    })
}

/// Each entry resets its SGR attributes so they cannot bleed into the next log line.
pub fn log_text(line: &LogLine) -> String {
    let color = match line.kind {
        LogKind::Error => "\x1b[91m",
        LogKind::System => "\x1b[96m",
        LogKind::Node => "",
    };
    format!("\x1b[2m{}\x1b[22m {color}{}\x1b[0m", line.time, line.text)
}
