//! In-place value editor for variables (text fields, switch, dropdown, duration, textarea, read-only preview).

use leptos::prelude::*;
use ticknode_shared::{DataType, DataValue};
use crate::i18n::{self, t};
use crate::inline::duration::units::{self, Unit};
use crate::state::use_editor;
use crate::ui::Switch;

#[component]
pub fn VariableValueEditor(
    data_type: DataType,
    #[prop(into)] value: Signal<DataValue>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let ed = use_editor();
    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    if ed.types.with_untracked(|types| types.definition_of(&data_type).is_some_and(|d| d.quantity().is_some())) {
        return view! {
            <crate::inline::InlineValue value=Signal::derive(move || Some(value.get())) value_type=data_type
                on_commit=Callback::new(move |v: Option<DataValue>| { if let Some(v) = v { on_commit.run(v); } }) />
        }.into_any();
    }

    // 1. Bool: Switch
    if data_type == DataType::Bool {
        let checked = Memo::new(move |_| matches!(value.get(), DataValue::Bool(true)));
        let on_toggle = Callback::new(move |b: bool| {
            on_commit.run(DataValue::Bool(b));
        });
        return view! {
            <div class="var-value-ctrl var-bool" on:pointerdown=stop>
                <Switch checked=Signal::derive(move || checked.get()) on_toggle=on_toggle small=true />
            </div>
        }.into_any();
    }

    // 2. Enum: select dropdown
    let enum_def = ed.types.with_untracked(|types| types.definition_of(&data_type).filter(|d| d.is_enum()).cloned());
    if let Some(def) = enum_def {
        let current_val = Memo::new(move |_| match value.get() {
            DataValue::String(s) => s,
            _ => String::new(),
        });
        let type_name = def.name.clone();
        return view! {
            <div class="var-value-ctrl" on:pointerdown=stop>
                <select
                    class="inline-value select var-select"
                    prop:value=move || current_val.get()
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        on_commit.run(DataValue::String(v));
                    }
                >
                    {def.values().iter().map(|v| {
                        let (type_name, enum_val) = (type_name.clone(), v.clone());
                        let label = move || i18n::enum_value_title(&type_name, &enum_val);
                        view! { <option value=v.value.clone()>{label}</option> }
                    }).collect_view()}
                </select>
            </div>
        }.into_any();
    }

    // 3. Duration: number input with unit select
    if data_type == DataType::Duration {
        return view! {
            <DurationEditor value=value on_commit=on_commit />
        }.into_any();
    }

    // 4. String: multiline textarea for templates / long texts, or single-line input with toggle
    if data_type == DataType::String {
        return view! {
            <StringEditor value=value on_commit=on_commit />
        }.into_any();
    }

    // 5. Text-form scalars: Number, Time, DateTime, Color
    if matches!(data_type, DataType::Number | DataType::Time | DataType::DateTime | DataType::Color) {
        return view! {
            <ScalarTextEditor data_type=data_type value=value on_commit=on_commit />
        }.into_any();
    }

    // 6. Complex types: List, Object, Any, Struct - read-only preview with expand toggle
    view! {
        <ComplexPreview value=value data_type=data_type />
    }.into_any()
}

#[component]
fn DurationEditor(
    #[prop(into)] value: Signal<DataValue>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let initial_val = value.get_untracked();
    let (initial_text, initial_unit) = units::presentation(Some(&initial_val), None);

    let draft = RwSignal::new(initial_text);
    let unit = RwSignal::new(initial_unit);
    let error = RwSignal::new(None::<String>);
    let is_focused = RwSignal::new(false);
    let is_dirty = RwSignal::new(false);

    Effect::new(move |_| {
        let v = value.get();
        if !is_focused.get_untracked() && !is_dirty.get_untracked() {
            let (t, u) = units::presentation(Some(&v), Some(unit.get_untracked()));
            draft.set(t);
            unit.set(u);
            error.set(None);
        }
    });

    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    let commit = move || {
        let text = draft.get_untracked();
        let u = unit.get_untracked();
        match units::parse(&text, u) {
            Ok(Some(parsed)) => {
                error.set(None);
                is_dirty.set(false);
                if parsed != value.get_untracked() {
                    on_commit.run(parsed);
                }
            }
            Ok(None) => {
                let (t, u) = units::presentation(Some(&value.get_untracked()), Some(u));
                draft.set(t);
                unit.set(u);
                error.set(None);
                is_dirty.set(false);
            }
            Err(msg) => {
                error.set(Some(i18n::ui_text(&msg)));
            }
        }
    };

    view! {
        <div class="var-value-ctrl var-duration-ctrl" on:pointerdown=stop>
            <div class="var-duration-row">
                <input
                    type="text"
                    class="inline-value duration-number var-duration-input"
                    prop:value=move || draft.get()
                    on:focus=move |_| is_focused.set(true)
                    on:blur=move |_| {
                        is_focused.set(false);
                        commit();
                    }
                    on:input=move |ev| {
                        is_dirty.set(true);
                        draft.set(event_target_value(&ev));
                        error.set(None);
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            commit();
                        } else if ev.key() == "Escape" {
                            let (t, u) = units::presentation(Some(&value.get_untracked()), Some(unit.get_untracked()));
                            draft.set(t);
                            unit.set(u);
                            error.set(None);
                            is_dirty.set(false);
                        }
                    }
                />
                <select
                    class="duration-unit var-duration-select"
                    prop:value=move || unit.get().id()
                    on:change=move |ev| {
                        if let Some(new_unit) = Unit::from_id(&event_target_value(&ev)) {
                            unit.set(new_unit);
                            commit();
                        }
                    }
                >
                    {Unit::ALL.into_iter().map(|u| {
                        view! { <option value=u.id()>{u.label()}</option> }
                    }).collect_view()}
                </select>
            </div>
            {move || error.get().map(|err| view! {
                <div class="var-field-error">{err}</div>
            })}
        </div>
    }
}

#[component]
fn StringEditor(
    #[prop(into)] value: Signal<DataValue>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let initial_str = match value.get_untracked() {
        DataValue::String(s) => s,
        _ => String::new(),
    };
    let initial_multiline = initial_str.contains('\n') || initial_str.chars().count() > 40;

    let draft = RwSignal::new(initial_str);
    let multiline = RwSignal::new(initial_multiline);
    let is_focused = RwSignal::new(false);
    let is_dirty = RwSignal::new(false);

    Effect::new(move |_| {
        let v = match value.get() {
            DataValue::String(s) => s,
            _ => String::new(),
        };
        if !is_focused.get_untracked() && !is_dirty.get_untracked() {
            draft.set(v);
        }
    });

    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    let commit = move || {
        let text = draft.get_untracked();
        is_dirty.set(false);
        if DataValue::String(text.clone()) != value.get_untracked() {
            on_commit.run(DataValue::String(text));
        }
    };

    let toggle_multiline = move |_| {
        multiline.update(|m| *m = !*m);
    };

    view! {
        <div class="var-value-ctrl var-string-ctrl" on:pointerdown=stop>
            <div class="var-string-row">
                {move || if multiline.get() {
                    view! {
                        <textarea
                            class="inline-value multiline var-textarea"
                            rows="3"
                            prop:value=move || draft.get()
                            on:focus=move |_| is_focused.set(true)
                            on:blur=move |_| {
                                is_focused.set(false);
                                commit();
                            }
                            on:input=move |ev| {
                                is_dirty.set(true);
                                draft.set(event_target_value(&ev));
                            }
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                if ev.key() == "Escape" {
                                    if let DataValue::String(s) = value.get_untracked() {
                                        draft.set(s);
                                    }
                                    is_dirty.set(false);
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                                        let _ = el.blur();
                                    }
                                } else if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                                    ev.prevent_default();
                                    commit();
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                                        let _ = el.blur();
                                    }
                                }
                            }
                        ></textarea>
                    }.into_any()
                } else {
                    view! {
                        <input
                            type="text"
                            class="inline-value var-input"
                            prop:value=move || draft.get()
                            on:focus=move |_| is_focused.set(true)
                            on:blur=move |_| {
                                is_focused.set(false);
                                commit();
                            }
                            on:input=move |ev| {
                                is_dirty.set(true);
                                draft.set(event_target_value(&ev));
                            }
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                if ev.key() == "Escape" {
                                    if let DataValue::String(s) = value.get_untracked() {
                                        draft.set(s);
                                    }
                                    is_dirty.set(false);
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                                        let _ = el.blur();
                                    }
                                } else if ev.key() == "Enter" {
                                    commit();
                                    use wasm_bindgen::JsCast;
                                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                                        let _ = el.blur();
                                    }
                                }
                            }
                        />
                    }.into_any()
                }}
                <button
                    class="btn-icon ctl var-toggle-multiline"
                    title=move || t!("variables-toggle-multiline")
                    on:click=toggle_multiline
                >
                    <span class="mdi mdi-format-line-spacing" aria-hidden="true"></span>
                </button>
            </div>
        </div>
    }
}

#[component]
fn ScalarTextEditor(
    data_type: DataType,
    #[prop(into)] value: Signal<DataValue>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let initial_text = i18n::value_edit_text(&value.get_untracked());
    let draft = RwSignal::new(initial_text);
    let error = RwSignal::new(None::<String>);
    let is_focused = RwSignal::new(false);
    let is_dirty = RwSignal::new(false);

    Effect::new(move |_| {
        let v = value.get();
        if !is_focused.get_untracked() && !is_dirty.get_untracked() {
            draft.set(i18n::value_edit_text(&v));
            error.set(None);
        }
    });

    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    let ty_for_commit = data_type.clone();
    let commit = Callback::new(move |()| {
        let text = draft.get_untracked();
        match i18n::parse_value_localized(&text, &ty_for_commit) {
            Ok(parsed) => {
                error.set(None);
                is_dirty.set(false);
                if parsed != value.get_untracked() {
                    on_commit.run(parsed);
                }
            }
            Err(err_msg) => {
                error.set(Some(err_msg));
            }
        }
    });

    let ty_for_esc = data_type.clone();
    view! {
        <div class="var-value-ctrl var-scalar-ctrl" on:pointerdown=stop>
            <input
                type="text"
                class="inline-value var-input"
                placeholder=move || i18n::type_hint(&ty_for_esc)
                prop:value=move || draft.get()
                on:focus=move |_| is_focused.set(true)
                on:blur=move |_| {
                    is_focused.set(false);
                    commit.run(());
                }
                on:input=move |ev| {
                    is_dirty.set(true);
                    draft.set(event_target_value(&ev));
                    error.set(None);
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        draft.set(i18n::value_edit_text(&value.get_untracked()));
                        error.set(None);
                        is_dirty.set(false);
                        use wasm_bindgen::JsCast;
                        if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                            let _ = el.blur();
                        }
                    } else if ev.key() == "Enter" {
                        commit.run(());
                    }
                }
            />
            {move || error.get().map(|err| view! {
                <div class="var-field-error">{err}</div>
            })}
        </div>
    }
}

#[component]
fn ComplexPreview(
    #[prop(into)] value: Signal<DataValue>,
    data_type: DataType,
) -> impl IntoView {
    let is_expanded = RwSignal::new(false);
    let full_text = Memo::new(move |_| crate::panels::display_value_in(&value.get(), &data_type));
    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    view! {
        <div class="var-complex-preview" on:pointerdown=stop>
            {move || {
                let text = full_text.get();
                if text.chars().count() > 50 {
                    let truncated: String = text.chars().take(50).collect();
                    view! {
                        <div class="var-preview-wrap">
                            {move || if is_expanded.get() {
                                view! { <pre class="var-preview-full">{text.clone()}</pre> }.into_any()
                            } else {
                                view! { <span class="var-preview">{format!("{truncated}…")}</span> }.into_any()
                            }}
                            <button
                                class="btn ctl btn-sm var-expand-btn"
                                on:click=move |_| is_expanded.update(|e| *e = !*e)
                            >
                                {move || if is_expanded.get() { t!("variables-collapse") } else { t!("variables-expand") }}
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <span class="var-preview">{text}</span>
                    }.into_any()
                }
            }}
        </div>
    }
}
