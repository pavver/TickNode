//! Inline values directly on a node: property rows below the header and unconnected input values
//! next to pins (like Factorio combinator parameters — visible without opening the inspector).

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::{self, t};
use crate::state::{ClientNode, Editor, LogKind, use_editor};
use crate::ui::{ControlState, flash};

mod properties;

pub use properties::*;

/// Display text for a field value. For `Any`, a string that looks like a number or `true`/`false` is quoted.
fn value_text(value: &DataValue, value_type: &DataType) -> String {
    match value {
        DataValue::String(s) if *value_type == DataType::Any && !matches!(text::parse_any(s), DataValue::String(_)) => format!("\"{s}\""),
        other => text::format_value(other),
    }
}

/// Parses typed text; `Ok(None)` means the field was cleared.
fn parse_text(input: &str, value_type: &DataType) -> Result<Option<DataValue>, String> {
    if input.trim().is_empty() && *value_type != DataType::String {
        return Ok(None);
    }
    i18n::parse_value_localized(input, value_type).map(Some)
}

/// Compact inline value field on a node. Commits on Enter or blur (Esc cancels).
/// Renders as a toggle for `Bool`. `optional`: the value can be cleared (empty field / third toggle state).
#[component]
pub fn InlineValue(
    #[prop(into)] value: Signal<Option<DataValue>>,
    value_type: DataType,
    on_commit: Callback<Option<DataValue>>,
    #[prop(optional)] optional: bool,
    #[prop(optional, into)] title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let state = RwSignal::new(ControlState::Idle);
    // Pressing on the field does not start node dragging or rubber-band selection
    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    if value_type == DataType::Bool {
        let toggle = move |_| {
            let next = match value.get_untracked() {
                Some(DataValue::Bool(true)) => Some(DataValue::Bool(false)),
                Some(DataValue::Bool(false)) if optional => None,
                _ => Some(DataValue::Bool(true)),
            };
            on_commit.run(next);
        };
        return view! {
            <button
                // Single combined class string: separate `class:` directives and dynamic `class` would overwrite each other
                class=move || match value.get() {
                    Some(DataValue::Bool(true)) => "inline-value bool on",
                    Some(DataValue::Bool(false)) => "inline-value bool",
                    _ => "inline-value bool unset",
                }
                title=move || title.get()
                on:pointerdown=stop
                on:click=toggle
            >
                {move || match value.get() {
                    Some(DataValue::Bool(b)) => b.to_string(),
                    _ => "—".to_string(),
                }}
            </button>
        }
        .into_any();
    }

    // Enum: value is chosen from a dropdown rather than typed
    let enum_def = ed.types.with_untracked(|types| types.definition_of(&value_type).filter(|d| d.is_enum()).cloned());
    if let Some(def) = enum_def {
        return view! {
            <select
                class="inline-value select"
                title=move || title.get()
                prop:value=move || match value.get() {
                    Some(DataValue::String(s)) => s,
                    _ => String::new(),
                }
                on:pointerdown=stop
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    on_commit.run((!v.is_empty()).then_some(DataValue::String(v)));
                }
            >
                {optional.then(|| view! { <option value="">"—"</option> })}
                {def.values().iter().map(|v| {
                    let (type_name, value) = (def.name.clone(), v.clone());
                    let label = move || i18n::enum_value_title(&type_name, &value);
                    view! { <option value=v.value.clone()>{label}</option> }
                }).collect_view()}
            </select>
        }
        .into_any();
    }

    let hint_type = value_type.clone();
    let shown_type = value_type.clone();
    let shown = Memo::new(move |_| value.with(|v| v.as_ref().map(|v| value_text(v, &shown_type)).unwrap_or_default()));
    let draft = RwSignal::new(String::new());
    Effect::new(move |_| draft.set(shown.get()));
    // The field may lose focus after it has already been removed from the DOM — in that case signals are disposed
    let commit = move || {
        let (Some(text), Some(saved)) = (draft.try_get_untracked(), shown.try_get_untracked()) else { return };
        if text == saved {
            return;
        }
        match parse_text(&text, &value_type) {
            Ok(None) if !optional => draft.set(saved),
            Ok(v) => on_commit.run(v),
            Err(e) => {
                draft.set(saved);
                flash(state, ControlState::Error);
                ed.log(LogKind::Error, format!("[ERROR] {e}"));
            }
        }
    };
    view! {
        <input
            type="text"
            class=move || format!("inline-value {}", state.get().class())
            placeholder=move || i18n::type_hint(&hint_type)
            title=move || title.get()
            spellcheck="false"
            prop:value=move || draft.get()
            on:pointerdown=stop
            on:input=move |ev| draft.set(event_target_value(&ev))
            on:focus=move |ev| {
                use wasm_bindgen::JsCast;
                if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                    input.select();
                }
            }
            on:blur=move |_| commit()
            on:keydown=move |ev| {
                use wasm_bindgen::JsCast;
                let input = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok());
                match ev.key().as_str() {
                    "Enter" => { input.map(|i| i.blur()); }
                    "Escape" => {
                        draft.set(shown.get_untracked());
                        input.map(|i| i.blur());
                    }
                    _ => {}
                }
            }
        />
    }
    .into_any()
}

fn property_value(node: ClientNode, name: &str) -> Option<DataValue> {
    node.instance.with(|i| i.model.properties.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()))
}

/// Value of an unconnected data input (rendered inside the pin row).
#[component]
pub fn PinValue(ed: Editor, node: NodeId, pin: PinId, value_type: DataType) -> impl IntoView {
    let Some(client) = ed.node(node) else { return ().into_any() };
    let name = input_value_property(pin);
    view! {
        <InlineValue
            value=Signal::derive(move || property_value(client, &name))
            value_type=value_type
            optional=true
            title=Signal::derive(move || t!("inline-pin-value-title"))
            on_commit=Callback::new(move |value| ed.send(ClientMessage::SetInputValue { id: node, pin, value }))
        />
    }
    .into_any()
}

#[cfg(test)]
mod tests;
