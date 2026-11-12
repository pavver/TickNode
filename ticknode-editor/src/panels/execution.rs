//! Last execution trace and pin values.

use leptos::prelude::*;
use ticknode_shared::*;

use super::formatting::{MAX_VALUE_CHARS, instance_pin_label, truncate};
use super::{display_value, relative_time};
use crate::geometry::pin_color;
use crate::i18n::{self, t};
use crate::state::use_editor;

/// When the node last executed and what values were on its pins.
#[component]
pub(super) fn ExecutionInfo(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else {
        return ().into_any();
    };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));
    let when = move || {
        let at = node.last_exec.get();
        if at.is_finite() {
            t!("exec-when", time = relative_time(ed.now.get() - at))
        } else {
            t!("exec-never")
        }
    };
    let tick = move || {
        node.trace
            .with(|tr| tr.as_ref().map(|tr| t!("exec-tick", tick = tr.tick)))
    };

    let rows = move |dir: PinDirection| {
        let pins: Vec<PinDefinition> = def.with(|d| {
            d.pins
                .iter()
                .filter(|p| p.direction == dir)
                .cloned()
                .collect()
        });
        pins.into_iter().map(|p| {
            let pin = p.id;
            let exec = p.pin_type == PinType::Exec;
            // (text, muted, tooltip)
            let value = move || -> (String, bool, String) {
                let connected = match dir {
                    PinDirection::Input => ed.is_input_connected(id, pin),
                    PinDirection::Output => ed.is_output_connected(id, pin),
                };
                node.trace.with(|trace| {
                    if !exec && let Some(v) = trace.as_ref().and_then(|t| crate::node_status::trace_value(t, pin, dir)) {
                        let full = super::display_value_in(v, &crate::node_status::trace_type(ed, id, pin, dir));
                        return (truncate(full.clone(), MAX_VALUE_CHARS), false, truncate(full, 5000));
                    }
                    trace_value(trace.as_ref(), dir, exec, pin, connected)
                })
            };
            let pin_type = p.pin_type.clone();
            let ty = move || match &pin_type {
                PinType::Exec => i18n::type_name_label("exec"),
                PinType::Data(t) => ed.type_title(t),
            };
            let technical_type = p.pin_type.clone();
            let technical = move || match &technical_type {
                PinType::Data(t) => t!("pin-technical-type", name = data_type_name(t)),
                _ => String::new(),
            };
            let raw_pin_name = p.name.clone();
            let pin_label = move || node.instance.with(|i| instance_pin_label(i, pin, dir, &raw_pin_name));
            view! {
                <div class="io-row">
                    <div class="dense-row">
                        <span class="socket connected" class:exec=exec style=format!("--pin-color:{}", ed.types.with_untracked(|types| pin_color(types, &p.pin_type)))></span>
                        <span class="ellipsis" style="flex:1">{pin_label}</span>
                        <span class="t-caption" title=technical>{ty}</span>
                    </div>
                    {move || {
                        let (text, muted, title) = value();
                        (!text.is_empty()).then(|| view! { <div class="io-value" class:muted=muted title=title>{text}</div> })
                    }}
                </div>
            }
        }).collect_view()
    };

    view! {
        <div class="section-title">{move || t!("exec-section-title")}</div>
        <div class="panel-section">
            <div class="dense-row">
                <span class="t-body-small" style="flex:1">{when}</span>
                <span class="t-caption">{tick}</span>
            </div>
        </div>
        <div class="section-title">{move || t!("exec-section-inputs")}</div>
        <div class="panel-section">{move || rows(PinDirection::Input)}</div>
        <div class="section-title">{move || t!("exec-section-outputs")}</div>
        <div class="panel-section">{move || rows(PinDirection::Output)}</div>
    }
    .into_any()
}

fn shown(value: &DataValue) -> (String, bool, String) {
    let full = display_value(value);
    (
        truncate(full.clone(), MAX_VALUE_CHARS),
        false,
        truncate(full, 5000),
    )
}

/// Text, muted state and tooltip of a pin in the last execution.
fn trace_value(
    trace: Option<&NodeTrace>,
    dir: PinDirection,
    exec: bool,
    pin: PinId,
    connected: bool,
) -> (String, bool, String) {
    let Some(trace) = trace else {
        return ("—".into(), true, t!("exec-node-never-hint"));
    };
    match (dir, exec) {
        (PinDirection::Input, true) => (String::new(), true, String::new()),
        (PinDirection::Output, true) if trace.fired.contains(&pin) => {
            (t!("exec-fired"), false, String::new())
        }
        (PinDirection::Output, true) => ("—".into(), true, t!("exec-not-fired-hint")),
        (PinDirection::Input, false) => input_trace_value(trace, pin, connected),
        (PinDirection::Output, false) => match trace.outputs.iter().find(|(p, _)| *p == pin) {
            Some((_, v)) => shown(v),
            None => ("—".into(), true, t!("exec-output-not-emitted-hint")),
        },
    }
}

fn input_trace_value(trace: &NodeTrace, pin: PinId, connected: bool) -> (String, bool, String) {
    match trace.inputs.iter().find(|(p, _)| *p == pin) {
        Some((_, Some(v))) => shown(v),
        Some((_, None)) => (t!("exec-no-value"), true, t!("exec-no-value-hint")),
        None if connected => ("—".into(), true, t!("exec-connected-after-hint")),
        None => (t!("exec-not-connected"), true, t!("exec-default-used-hint")),
    }
}

#[cfg(test)]
mod tests;
