//! Helpers and components for binding unconnected data inputs to variables.

use leptos::prelude::*;
use std::collections::BTreeMap;
use ticknode_shared::*;

use super::parse_text;
use crate::i18n::{self, t};
use crate::state::{Editor, LogKind};
use crate::ui::{ControlState, flash};

/// Outcome of committing text in an unconnected input or variable field.
#[derive(Debug, Clone, PartialEq)]
pub enum InputCommit {
    Value(Option<DataValue>),
    Variable(String),
}

/// Typed fields commit a variable reference before trying their unit-specific literal parser.
pub(super) fn commit_variable_binding(
    text: &str,
    callback: Option<Callback<Option<String>>>,
) -> bool {
    let Some(callback) = callback else {
        return false;
    };
    let text = text.trim();
    if VariableRef::parse(text).is_none() {
        return false;
    }
    callback.run(Some(text.to_string()));
    true
}

/// Decides whether committed text represents a variable binding or a literal value.
///
/// When `allow_variable` is true:
/// - text parsing via `VariableRef::parse` becomes `InputCommit::Variable`
/// - for String inputs, a quoted literal `"var.x"` strips one pair of surrounding double quotes
/// - empty / whitespace input clears the field (`InputCommit::Value(None)`)
pub fn decide_input_commit(
    raw_text: &str,
    value_type: &DataType,
    optional: bool,
    allow_variable: bool,
) -> Result<InputCommit, String> {
    let trimmed = raw_text.trim();
    if clears_input(trimmed, value_type, optional) {
        return Ok(InputCommit::Value(None));
    }
    if *value_type == DataType::String
        && allow_variable
        && let Some(literal) = quoted_literal(trimmed)
    {
        return Ok(InputCommit::Value(Some(literal.into())));
    }
    if allow_variable && VariableRef::parse(trimmed).is_some() {
        return Ok(InputCommit::Variable(trimmed.to_string()));
    }
    if *value_type == DataType::String {
        return Ok(InputCommit::Value(Some(raw_text.into())));
    }

    parse_text(raw_text, value_type).map(InputCommit::Value)
}

fn clears_input(text: &str, value_type: &DataType, optional: bool) -> bool {
    text.is_empty() && (*value_type != DataType::String || optional)
}

fn quoted_literal(text: &str) -> Option<&str> {
    text.strip_prefix('"')?.strip_suffix('"')
}

/// Resolves a variable reference against open sheet variables first (shadowing), then global/persistent.
pub fn resolve_variable<'a>(
    vref: &VariableRef,
    open_sheet: Option<SheetId>,
    variables: &'a BTreeMap<VariableKey, Variable>,
) -> Option<&'a Variable> {
    match vref.lookup {
        VariableLookup::Plugin => None,
        VariableLookup::Auto => {
            if let Some(sheet_id) = open_sheet {
                let key = VariableKey::sheet(sheet_id, &vref.name);
                if let Some(var) = variables.get(&key) {
                    return Some(var);
                }
            }
            let global_key = VariableKey::global(&vref.name);
            if let Some(var) = variables.get(&global_key) {
                return Some(var);
            }
            let pers_key = VariableKey::persistent(&vref.name);
            variables.get(&pers_key)
        }
        VariableLookup::Sheet => {
            let sheet_id = open_sheet?;
            let key = VariableKey::sheet(sheet_id, &vref.name);
            variables.get(&key)
        }
        VariableLookup::Global => {
            let global_key = VariableKey::global(&vref.name);
            if let Some(var) = variables.get(&global_key) {
                return Some(var);
            }
            let pers_key = VariableKey::persistent(&vref.name);
            variables.get(&pers_key)
        }
    }
}

/// Reads an exact variable name or a typed path inside its value.
pub fn resolve_variable_value<'a>(
    reference: &VariableRef,
    sheet: Option<SheetId>,
    variables: &'a BTreeMap<VariableKey, Variable>,
    types: &TypeRegistry,
) -> Option<(&'a DataValue, DataType)> {
    let mut end = reference.name.len();
    loop {
        let root = VariableRef::new(reference.lookup, &reference.name[..end]);
        if let Some(var) = resolve_variable(&root, sheet, variables) {
            let path = reference.name[end..].trim_start_matches('.');
            return Some((
                plugin_values::project(&var.value, path)?,
                effective_type(Some(&var.value), &var.data_type, path, types)?,
            ));
        }
        end = reference.name[..end].rfind('.')?;
    }
}

/// Builds the tooltip string for a bound variable chip.
pub fn variable_tooltip(
    vref: &VariableRef,
    resolved: Option<(&DataValue, &DataType)>,
    types: &TypeRegistry,
) -> String {
    match resolved {
        Some((value, ty)) => {
            let type_name = i18n::data_type_title(types, ty);
            let val_str = i18n::value_as_text(value, ty);
            t!(
                "inline-variable-tooltip",
                name = vref.name.clone(),
                type = type_name,
                value = val_str
            )
        }
        None => t!("inline-variable-not-found", name = vref.name.clone()),
    }
}

/// In-place text editor opened by clicking a bound variable chip.
#[component]
pub fn BoundInputEditor(
    ed: Editor,
    node: NodeId,
    pin: PinId,
    value_type: DataType,
    initial: String,
    on_finish: Callback<()>,
) -> impl IntoView {
    let draft = RwSignal::new(initial.clone());
    let state = RwSignal::new(ControlState::Idle);
    let saved = initial;
    let hint_type = value_type.clone();

    let commit = move || {
        let (Some(text), Some(saved)) = (draft.try_get_untracked(), Some(saved.clone())) else {
            return;
        };
        if text == saved {
            on_finish.run(());
            return;
        }
        match decide_input_commit(&text, &value_type, true, true) {
            Ok(InputCommit::Variable(var_name)) => {
                ed.send(ClientMessage::SetInputVariable {
                    id: node,
                    pin,
                    variable: Some(var_name),
                });
                on_finish.run(());
            }
            Ok(InputCommit::Value(val)) => {
                if val.is_none() {
                    ed.send(ClientMessage::SetInputVariable {
                        id: node,
                        pin,
                        variable: None,
                    });
                } else {
                    ed.send(ClientMessage::SetInputValue {
                        id: node,
                        pin,
                        value: val,
                    });
                }
                on_finish.run(());
            }
            Err(_) => {
                flash(state, ControlState::Error);
                ed.log(
                    LogKind::Error,
                    format!(
                        "[ERROR] Invalid inline value for {}",
                        data_type_name(&value_type)
                    ),
                );
            }
        }
    };

    view! {
        <input
            type="text"
            autofocus=true
            class=move || format!("inline-value {}", state.get().class())
            placeholder=move || i18n::type_hint(&hint_type)
            title=move || t!("inline-variable-edit-title")
            spellcheck="false"
            prop:value=move || draft.get()
            on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
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
                        on_finish.run(());
                    }
                    _ => {}
                }
            }
        />
    }
}
