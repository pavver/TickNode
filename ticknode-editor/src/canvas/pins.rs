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
    let value_type = ed.types.with_untracked(|types| inline_type(types, dir, &pin.pin_type));
    let value = value_type.map(|t| move || (!connected()).then(|| view! { <PinValue ed=ed node=node pin=pin_id value_type=t.clone() /> }));
    let pin_name_title = pin.name.clone();
    let pin_type_title = pin.pin_type.clone();
    let pin_title = move || {
        let translated_pin = translated_pin_name(ed, node, &pin_name_title);
        let type_name = pin_type_title_label(ed, &pin_type_title);
        pin_tooltip(&translated_pin, &type_name, &pin_type_title, is_required)
    };
    let pending = use_context::<PendingConnection>();
    let target_type = pin.pin_type.clone();
    let allowed_target = move || target_conversion(ed, pending?.0.get()?, node, dir, &target_type);
    let split_control_pin = pin.clone();
    let is_field = split_field_parent(pin.id).is_some();
    let pin_name_label = pin.name;
    let pin_label = move || match label {
        Some(custom) => custom.get(),
        None => translated_pin_name(ed, node, &pin_name_label),
    };
    view! {
        <div
            // Single-string class attribute: a dynamic `class` would overwrite separate `class:` bindings
            class=move || {
                pin_class(is_input, is_field, is_input && missing(), allowed_target().is_some())
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

fn translated_pin_name(ed: crate::state::Editor, node: NodeId, name: &str) -> String {
    ed.node_tracked(node).map_or_else(|| name.to_string(), |n| n.instance.with(|i| i18n::node_pin_label(&i.definition.type_name, name)))
}

fn pin_type_title_label(ed: crate::state::Editor, pin: &PinType) -> String {
    match pin { PinType::Exec => i18n::type_name_label("exec"), PinType::Data(t) => ed.type_title(t) }
}

fn pin_tooltip(name: &str, type_name: &str, pin: &PinType, required: bool) -> String {
    let required_note = if required { t!("pin-required-suffix") } else { String::new() };
    let technical = match pin {
        PinType::Data(t) => format!("\n{}", t!("pin-technical-type", name = data_type_name(t))),
        _ => String::new(),
    };
    format!("{name}: {type_name}{required_note}{technical}")
}

fn inline_type(types: &TypeRegistry, dir: PinDirection, pin: &PinType) -> Option<DataType> {
    match pin {
        PinType::Data(t) if dir == PinDirection::Input && input_value_editable(types, t) => Some(t.clone()),
        _ => None,
    }
}

fn target_conversion(ed: crate::state::Editor, from: super::wires::WireEnd, node: NodeId, dir: PinDirection, target: &PinType) -> Option<bool> {
    if from.dir == dir || from.node == node { return None; }
    let other = connection_pin(ed, from.node, from.pin, from.dir)?;
    let (src, dst) = if dir == PinDirection::Input { (&other, target) } else { (target, &other) };
    ed.types.with(|types| wire_conversion(types, src, dst))
}

fn pin_class(is_input: bool, is_field: bool, missing: bool, connectable: bool) -> String {
    let mut class = if is_input { "pin in" } else { "pin out" }.to_string();
    if is_field { class.push_str(" split-field"); }
    if missing { class.push_str(" missing"); }
    if connectable { class.push_str(" connectable"); }
    class
}

#[cfg(test)]
mod tests;
