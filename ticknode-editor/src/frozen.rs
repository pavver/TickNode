//! Presentation of unconfirmed pin contracts, shared by headers and the inspector.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::{self, t};
use crate::state::{Editor, use_editor};

fn direction(dir: PinDirection) -> String {
    match dir {
        PinDirection::Input => t!("pin-direction-input"),
        PinDirection::Output => t!("pin-direction-output"),
    }
}

pub fn change_text(ed: Editor, def: &NodeDefinition, change: &PinChange) -> String {
    format_change(&def.type_name, change, |ty| ed.type_title(ty))
}

fn format_change(
    type_name: &str,
    change: &PinChange,
    type_title: impl Fn(&DataType) -> String,
) -> String {
    let kind = |pin: &PinSignature| match &pin.kind {
        PinType::Exec => t!("type-exec"),
        PinType::Data(ty) => type_title(ty),
    };
    let name = |pin: &PinSignature| i18n::node_pin_label(type_name, &pin.name);
    match change {
        PinChange::Removed { pin } => t!(
            "pin-change-removed",
            direction = direction(pin.direction),
            name = name(pin),
            kind = kind(pin)
        ),
        PinChange::Changed { old, new } => t!(
            "pin-change-changed",
            name = name(old),
            old = format!("{} · {}", direction(old.direction), kind(old)),
            new = format!("{} · {}", direction(new.direction), kind(new))
        ),
        PinChange::RequiredAdded { pin } => {
            t!("pin-change-required", name = name(pin), kind = kind(pin))
        }
    }
}

#[component]
pub fn ConfirmPins(id: NodeId, #[prop(default = false)] compact: bool) -> impl IntoView {
    let ed = use_editor();
    view! {
        <button class=if compact { "node-confirm" } else { "btn ctl st-idle full" }
            title=move || t!("node-frozen-confirm")
            aria-label=move || t!("node-frozen-confirm")
            on:pointerdown=crate::match_editor::stop_on_controls
            on:click=move |_| ed.send(ClientMessage::ConfirmPins(id))>
            <span class="btn-content">
                <span class="mdi mdi-check" aria-hidden="true"></span>
                {move || (!compact).then(|| t!("node-frozen-confirm"))}
            </span>
        </button>
    }
}

#[component]
pub fn FrozenInfo(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    move || {
        let node = ed.node_tracked(id)?;
        let instance = node.instance.get();
        if instance.frozen.is_empty() {
            return None;
        }
        Some(view! {
            <div class="section-title">{move || t!("node-frozen-title")}</div>
            <div class="panel-section frozen-info">
                <span class="t-body-small">{move || t!("node-frozen-help")}</span>
                <ul>{instance.frozen.iter().map(|change| view! {
                    <li class="t-body-small">{change_text(ed, &instance.definition, change)}</li>
                }).collect_view()}</ul>
                <ConfirmPins id=id />
            </div>
        })
    }
}

#[cfg(test)]
mod tests;
