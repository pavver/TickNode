//! Node frame, header and body.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::categories::{category_color, category_label};
use crate::geometry::*;
use crate::i18n::{self, t};
use crate::inline::NodeProps;
use crate::match_editor::MatchBody;
use crate::state::{EXEC_HIGHLIGHT_MS, use_editor};
use crate::ui::{Icon, icons};
use super::pins::Pin;

mod timer;
mod resize;

/// Pin used to trigger a node manually: first exec input; for event sources (Clock, button),
/// their exec output.
pub fn trigger_pin(def: &NodeDefinition) -> Option<PinId> {
    let exec = |dir| def.pins.iter().find(|p| p.direction == dir && p.pin_type == PinType::Exec).map(|p| p.id);
    exec(PinDirection::Input).or_else(|| exec(PinDirection::Output))
}

#[component]
pub(super) fn NodeView(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.nodes.with_untracked(|n| n.get(&id).copied()) else {
        return ().into_any();
    };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));
    let layout = Memo::new(move |_| ed.layout_of(node));
    let is_match = move || def.with(|d| d.type_name == "match");
    let style = move || {
        let p = node.pos.get();
        def.with(|d| format!(
            "left:{}px;top:{}px;width:{}px;height:{}px;--node-width:{}px;--default-node-width:{}px;--cat-color:{}",
            p.x, p.y, layout.with(|l| l.width), layout.with(|l| l.height), layout.with(|l| l.width), node_width(&d.type_name), category_color(&d.category)
        ))
    };
    let pins_style = move || format!("top:{}px", layout.with(|l| l.pins_top));
    let pin_list = move |dir: PinDirection| {
        def.with(|d| pins(d, dir).cloned().collect::<Vec<_>>())
            .into_iter()
            .map(|p| view! { <Pin node=id pin=p dir=dir /> })
            .collect_view()
    };
    // Stub node: its plugin is unavailable — the node is dimmed and properties are read-only
    let unavailable = Memo::new(move |_| node.instance.with(|i| i.unavailable.clone()));
    let frozen = Memo::new(move |_| node.instance.with(|i| !i.frozen.is_empty()));
    let trigger = Memo::new(move |_| if frozen.get() || unavailable.with(Option::is_some) { None } else { def.with(super::trigger_pin) });
    view! {
        <div
            class=move || {
                let mut class = "node ctl".to_string();
                if frozen.get() { class.push_str(" frozen"); }
                if unavailable.with(Option::is_some) { class.push_str(" unavailable"); }
                if def.with(|d| d.pins.iter().any(|p| ed.required_missing(id, p))) { class.push_str(" missing"); }
                if ed.is_selected(id) { class.push_str(" selected"); }
                if ed.now.get() - node.last_exec.get() < EXEC_HIGHLIGHT_MS { class.push_str(" active"); }
                if !frozen.get() && ed.node_errors.with(|errors| errors.contains_key(&id)) { class.push_str(" st-error"); }
                class
            }
            title=move || unavailable.get().map(|m| i18n::ui_text(&m)).unwrap_or_default()
            data-node=id.0.to_string()
            style=style
        >
            <div class="node-header" on:pointerdown=crate::match_editor::stop_on_controls>
                <span class="node-title ellipsis">{move || def.with(i18n::node_title)}</span>
                <span class="node-cat ellipsis" class:node-unavailable=move || unavailable.with(Option::is_some)>
                    {move || if unavailable.with(Option::is_some) { t!("node-plugin-unavailable") } else { def.with(|d| category_label(&d.category)) }}
                </span>
                {move || (!frozen.get() && !ed.node_errors.with(|errors| errors.contains_key(&id)) && def.with(|d| d.pins.iter().any(|p| ed.required_missing(id, p)))).then(|| view! {
                    <span class="node-warn" title=move || t!("node-missing-required-title")>"!"</span>
                })}
                <Show when=move || !frozen.get() && ed.node_errors.with(|errors| errors.contains_key(&id))>
                    <button class="node-error"
                        on:pointerenter=move |ev| crate::node_status::capture_title(&ev, || crate::node_status::error_text(ed, id))
                        aria-label=move || crate::node_status::error_text(ed, id)
                        on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
                        on:click=|ev| ev.stop_propagation()>
                        <span class="mdi mdi-alert-circle" aria-hidden="true"></span>
                    </button>
                </Show>
                <Show when=move || frozen.get()>
                    <span class="mdi mdi-alert-outline node-frozen-warning" title=move || node.instance.with(|i| {
                        i.frozen.iter().map(|change| crate::frozen::change_text(ed, &i.definition, change)).collect::<Vec<_>>().join("\n")
                    })></span>
                    <crate::frozen::ConfirmPins id=id compact=true />
                </Show>
                {move || trigger.get().map(|pin| view! {
                    // For ClickButton, triggering is its entire purpose, so "run" is always visible; for others, on hover
                    <button
                        class="node-run"
                        class:always=move || def.with(|d| d.type_name == "test/click_button")
                        title=move || t!("node-run-title")
                        on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
                        on:click=move |_| ed.send(ClientMessage::TriggerNode { id, pin })
                    >
                        <Icon path=icons::PLAY />
                    </button>
                })}
            </div>
            <NodeProps id=id />
            // Match renders its own rows: named/typed inputs followed by cases
            {move || if is_match() {
                view! { <MatchBody id=id top=Signal::derive(move || layout.with(|l| l.pins_top)) /> }.into_any()
            } else {
                view! {
                    <div class="pins in" style=pins_style>{move || pin_list(PinDirection::Input)}</div>
                    <div class="pins out" style=pins_style>{move || pin_list(PinDirection::Output)}</div>
                }.into_any()
            }}
            <timer::TimerBar id=id />
            <Show when=move || unavailable.with(Option::is_none)>
                <resize::ResizeHandle id=id />
            </Show>
        </div>
    }
    .into_any()
}
