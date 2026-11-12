//! A layout-neutral delay indicator, animated between server telemetry updates.

use crate::i18n::t;
use crate::state::{now_ms, use_editor};
use leptos::prelude::*;
use ticknode_shared::NodeId;

#[component]
pub(super) fn TimerBar(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let timer = Memo::new(move |_| ed.timers.with(|all| all.get(&id).cloned()));
    // Replacing the element restarts its CSS animation when this node's timer changes.
    {
        move || {
            timer.get().map(|timer| {
        let (remaining, fraction) = timer.progress(now_ms());
        let count = timer.timer.count;
        view! {
            <div class="node-timer" role="img" aria-label=move || t!("node-timer-active", count = count)>
                <div class="node-timer-fill" style=format!("--timer-fraction:{fraction};animation-duration:{remaining}ms")></div>
                {(count > 1).then(|| view! { <span class="node-timer-count">{move || t!("node-timer-count", count = count)}</span> })}
            </div>
        }
    })
        }
    }
}
