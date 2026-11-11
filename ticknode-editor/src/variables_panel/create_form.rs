//! Inline form for creating a new variable.

use leptos::prelude::*;
use ticknode_shared::{
    ClientMessage, DataType, SheetId, Variable, VariableKey, VariableScope,
    parse_data_type, text, valid_variable_name,
};
use crate::i18n::t;
use crate::inline::properties::{TYPE_OPTIONS, type_options};
use crate::state::use_editor;
use super::value_editor::VariableValueEditor;

#[component]
pub fn CreateVariableForm(
    sheet: Option<SheetId>,
    on_close: Callback<()>,
) -> impl IntoView {
    let ed = use_editor();
    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    let initial_scope = if sheet.is_some() {
        VariableScope::Sheet
    } else {
        VariableScope::Global
    };
    let scope = RwSignal::new(initial_scope);
    let name = RwSignal::new(String::new());
    let name_error = RwSignal::new(None::<String>);

    let available_types = type_options(ed, &TYPE_OPTIONS);
    let initial_type_name = available_types.first().map(|(val, _)| val.clone()).unwrap_or_else(|| "number".to_string());
    let selected_type_name = RwSignal::new(initial_type_name.clone());

    let initial_data_type = parse_data_type(&initial_type_name).unwrap_or(DataType::Number);
    let data_type = RwSignal::new(initial_data_type.clone());
    let value = RwSignal::new(text::default_of(&initial_data_type));

    let on_type_change = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        selected_type_name.set(val.clone());
        if let Some(parsed_ty) = parse_data_type(&val) {
            value.set(ed.types.with_untracked(|types| types.default_value(&parsed_ty)));
            data_type.set(parsed_ty);
        }
    };

    let submit = move || {
        let n = name.get_untracked();
        let trimmed = n.trim();
        if !valid_variable_name(trimmed) {
            name_error.set(Some(t!("error-variable-name", name = n)));
            return;
        }

        let key = match scope.get_untracked() {
            VariableScope::Sheet => {
                if let Some(sheet_id) = sheet {
                    VariableKey::sheet(sheet_id, trimmed)
                } else {
                    VariableKey::global(trimmed)
                }
            }
            VariableScope::Global => VariableKey::global(trimmed),
            VariableScope::Persistent => VariableKey::persistent(trimmed),
        };

        ed.send(ClientMessage::CreateVariable(Variable {
            key,
            data_type: data_type.get_untracked(),
            value: value.get_untracked(),
        }));
        on_close.run(());
    };

    view! {
        <div class="var-new-form" on:pointerdown=stop>
            <div class="var-new-field">
                <label class="field-label">{move || t!("variables-scope")}</label>
                <select
                    class="inline-value select var-new-select"
                    prop:value=move || match scope.get() {
                        VariableScope::Sheet => "sheet",
                        VariableScope::Global => "global",
                        VariableScope::Persistent => "persistent",
                    }
                    on:change=move |ev| {
                        match event_target_value(&ev).as_str() {
                            "sheet" => scope.set(VariableScope::Sheet),
                            "persistent" => scope.set(VariableScope::Persistent),
                            _ => scope.set(VariableScope::Global),
                        }
                    }
                >
                    {sheet.is_some().then(|| view! {
                        <option value="sheet">{move || t!("variables-scope-sheet-short")}</option>
                    })}
                    <option value="global">{move || t!("variables-scope-global-short")}</option>
                    <option value="persistent">{move || t!("variables-scope-persistent-short")}</option>
                </select>
            </div>

            <div class="var-new-field">
                <label class="field-label">{move || t!("variables-name")}</label>
                <input
                    type="text"
                    class="inline-value var-input"
                    placeholder=move || t!("variables-name")
                    prop:value=move || name.get()
                    on:input=move |ev| {
                        name.set(event_target_value(&ev));
                        name_error.set(None);
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            submit();
                        } else if ev.key() == "Escape" {
                            on_close.run(());
                        }
                    }
                />
                {move || name_error.get().map(|err| view! {
                    <div class="var-field-error">{err}</div>
                })}
            </div>

            <div class="var-new-field">
                <label class="field-label">{move || t!("variables-type")}</label>
                <select
                    class="inline-value select var-new-select"
                    prop:value=move || selected_type_name.get()
                    on:change=on_type_change
                >
                    {available_types.into_iter().map(|(val, label)| {
                        view! { <option value=val>{label}</option> }
                    }).collect_view()}
                </select>
            </div>

            <div class="var-new-field">
                <label class="field-label">{move || t!("variables-initial-value")}</label>
                {move || {
                    let cur_type = data_type.get();
                    let cur_val = value.get();
                    view! {
                        <VariableValueEditor
                            data_type=cur_type
                            value=Signal::derive(move || cur_val.clone())
                            on_commit=Callback::new(move |new_val| value.set(new_val))
                        />
                    }
                }}
            </div>

            <div class="var-new-actions">
                <button class="btn primary ctl btn-sm" on:click=move |_| submit()>
                    <span>{move || t!("variables-create")}</span>
                </button>
                <button class="btn ctl btn-sm" on:click=move |_| on_close.run(())>
                    <span>{move || t!("variables-cancel")}</span>
                </button>
            </div>
        </div>
    }
}
