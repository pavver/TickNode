//! Pin sockets, inline input values and struct split controls.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::*;
use crate::i18n::{self, t};
use crate::inline::PinValue;
use crate::state::use_editor;
use super::wires::{PendingConnection, connection_pin};

/// Sends the change to the server; effective pins arrive in NodeUpdated.
#[component]
pub fn PinSplitControl(node: NodeId, pin: PinDefinition) -> impl IntoView {
    let ed = use_editor();
    let parent = split_field_parent(pin.id).map(|(parent, _)| parent);
    let can_split = ed.types.with_untracked(|types| splittable_type(types, &pin).is_some());
    (parent.is_some() || can_split).then(|| view! {
        <button
            class="mini-btn icon pin-split"
            title=move || if parent.is_some() { t!("pin-collapse") } else { t!("pin-split") }
            aria-label=move || if parent.is_some() { t!("pin-collapse") } else { t!("pin-split") }
            disabled=move || ed.node_tracked(node).is_none_or(|n| n.instance.with(|i| i.unavailable.is_some()))
            on:pointerdown=crate::match_editor::stop_on_controls
            on:click=move |ev| {
                ev.stop_propagation();
                ed.send(ClientMessage::SetPinSplit { node, pin: parent.unwrap_or(pin.id), split: parent.is_none() });
            }
        >{if parent.is_some() { "⊟" } else { "⊞" }}</button>
    })
}

#[component]
pub fn Pin(
    node: NodeId,
    pin: PinDefinition,
    dir: PinDirection,
    /// Custom label overriding the pin name.
    #[prop(optional, into)]
    label: Option<Signal<String>>,
) -> impl IntoView {
    let ed = use_editor();
    let exec = pin.pin_type == PinType::Exec;
    let pin_id = pin.id;
    let connected = move || match dir {
        PinDirection::Input => ed.is_input_connected(node, pin_id),
        PinDirection::Output => ed.is_output_connected(node, pin_id),
    };
    let color = ed.types.with_untracked(|types| pin_color(types, &pin.pin_type));
    let is_input = dir == PinDirection::Input;
    let is_required = pin.required;
    let pin_def = pin.clone();
    let missing = move || ed.required_missing(node, &pin_def);
    // An unconnected data input can have an inline value set right here
    let value = match &pin.pin_type {
        PinType::Data(t) if is_input && ed.types.with_untracked(|types| input_value_editable(types, t)) => {
            let t = t.clone();
            Some(move || (!connected()).then(|| view! { <PinValue ed=ed node=node pin=pin_id value_type=t.clone() /> }))
        }
        _ => None,
    };
    let pin_name_title = pin.name.clone();
    let pin_type_title = pin.pin_type.clone();
    let pin_title = move || {
        let translated_pin = ed.node_tracked(node).map_or_else(
            || pin_name_title.clone(),
            |n| n.instance.with(|i| i18n::node_pin_label(&i.definition.type_name, &pin_name_title)),
        );
        let type_name = match &pin_type_title {
            PinType::Exec => i18n::type_name_label("exec"),
            PinType::Data(t) => ed.type_title(t),
        };
        let required_note = if is_required { t!("pin-required-suffix") } else { String::new() };
        let technical = match &pin_type_title {
            PinType::Data(t) => format!("\n{}", t!("pin-technical-type", name = data_type_name(t))),
            _ => String::new(),
        };
        format!("{translated_pin}: {type_name}{required_note}{technical}")
    };
    let pending = use_context::<PendingConnection>();
    let target_type = pin.pin_type.clone();
    let allowed_target = move || {
        let from = pending?.0.get()?;
        if from.dir == dir || from.node == node { return None; }
        let other = connection_pin(ed, from.node, from.pin, from.dir)?;
        let (src, dst) = if dir == PinDirection::Input { (&other, &target_type) } else { (&target_type, &other) };
        ed.types.with(|types| wire_conversion(types, src, dst))
    };
    let split_control_pin = pin.clone();
    let is_field = split_field_parent(pin.id).is_some();
    let pin_name_label = pin.name;
    let pin_label = move || match label {
        Some(custom) => custom.get(),
        None => ed.node_tracked(node).map_or_else(
            || pin_name_label.clone(),
            |n| n.instance.with(|i| i18n::node_pin_label(&i.definition.type_name, &pin_name_label)),
        ),
    };
    view! {
        <div
            // Single-string class attribute: a dynamic `class` would overwrite separate `class:` bindings
            class=move || {
                let mut class = if is_input { "pin in" } else { "pin out" }.to_string();
                if is_field { class.push_str(" split-field"); }
                if is_input && missing() { class.push_str(" missing"); }
                if allowed_target().is_some() { class.push_str(" connectable"); }
                class
            }
            title=pin_title
            data-pin=pin_id.0.to_string()
            data-node=node.0.to_string()
            data-dir=if is_input { "in" } else { "out" }
            data-kind=if exec { "exec" } else { "data" }
        >
            <span class="socket" class:exec=exec class:connected=connected style=format!("--pin-color:{color}")></span>
            <span class="ellipsis">{pin_label}</span>
            <PinSplitControl node=node pin=split_control_pin />
            {value}
        </div>
    }
}
