//! Значення прямо на ноді: рядки властивостей під заголовком і значення непід'єднаних входів
//! біля пінів (як параметри комбінаторів Factorio — видно, не відкриваючи інспектор).

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::is_inline_property;
use crate::i18n::{self, t};
use crate::match_editor::stop_on_controls;
use crate::panels::property_label;
use crate::state::{ClientNode, Editor, LogKind, use_editor};
use crate::ui::{ControlState, Switch, flash};

/// Текст значення в полі. Для Any рядок, схожий на число чи true/false, береться в лапки.
fn value_text(value: &DataValue, value_type: &DataType) -> String {
    match value {
        DataValue::String(s) if *value_type == DataType::Any && !matches!(text::parse_any(s), DataValue::String(_)) => format!("\"{s}\""),
        other => text::format_value(other),
    }
}

/// Розбирає набраний текст; `Ok(None)` — поле очищено.
fn parse_text(input: &str, value_type: &DataType) -> Result<Option<DataValue>, String> {
    if input.trim().is_empty() && *value_type != DataType::String {
        return Ok(None);
    }
    i18n::parse_value_localized(input, value_type).map(Some)
}

/// Компактне поле значення на ноді. Зміна застосовується по Enter чи втраті фокусу (Esc — скасувати).
/// Для `Bool` — перемикач. `optional`: значення можна прибрати (очистити поле / третій стан перемикача).
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
    // Натискання на поле не починає перетягування ноди чи рамку виділення
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
                // Увесь клас одним рядком: окремі `class:` і динамічний `class` перезаписують одне одного
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

    // Перелік: значення вибирається зі списку, а не набирається
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
                    let label = if v.title.is_empty() { v.value.clone() } else { v.title.clone() };
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
    // Поле може втратити фокус уже після того, як його прибрали з DOM — тоді сигнали знищені
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

/// Типи, які можна обрати для `*_type` (елементи списку, поле об'єкта); власні типи додаються з реєстру.
const TYPE_OPTIONS: [&str; 8] = ["number", "string", "bool", "time", "datetime", "duration", "color", "object"];
/// Типи значення константи.
const CONST_TYPES: [&str; 7] = ["number", "string", "bool", "time", "datetime", "duration", "color"];

fn default_value(type_name: &str) -> DataValue {
    match parse_data_type(type_name) {
        Some(DataType::Bool) => DataValue::Bool(false),
        Some(t) => text::default_of(&t),
        None => DataValue::Number(NumberValue::Int(0)),
    }
}

/// Пари (значення, підпис): вбудовані типи й власні типи плагінів (підпис — `title`).
fn type_options(ed: Editor, builtin: &[&str]) -> Vec<(String, String)> {
    let mut options: Vec<_> = builtin.iter().map(|o| (o.to_string(), i18n::type_name_label(o))).collect();
    ed.types.with_untracked(|types| {
        options.extend(types.definitions().into_iter().map(|d| (d.name.clone(), format!("{} ({})", d.title, d.name))));
    });
    options
}

/// Компактний випадний список на ноді.
#[component]
fn InlineSelect(
    mut options: Vec<(String, String)>,
    #[prop(into)] selected: Signal<String>,
    on_select: Callback<String>,
    #[prop(optional, into)] title: Signal<String>,
) -> impl IntoView {
    // Значення, якого немає в списку (тип зниклого плагіна), лишається видимим
    let current = selected.get_untracked();
    if !current.is_empty() && !options.iter().any(|(v, _)| *v == current) {
        options.push((current.clone(), current));
    }
    view! {
        <select
            class="inline-value select"
            title=move || title.get()
            prop:value=move || selected.get()
            on:change=move |ev| {
                let v = event_target_value(&ev);
                if v != selected.get_untracked() {
                    on_select.run(v);
                }
            }
        >
            {options.into_iter().map(|(value, label)| view! { <option value=value>{label}</option> }).collect_view()}
        </select>
    }
}

/// Рядки властивостей між заголовком ноди і пінами (див. `geometry::is_inline_property`):
/// типи — випадним списком, логічні — перемикачем, решта — полем. У константи поруч зі значенням
/// ще й вибір його типу.
#[component]
pub fn NodeProps(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else { return ().into_any() };
    let names = Memo::new(move |_| node.instance.with(|i| {
        i.model.properties.iter()
            .filter(|(n, v)| is_inline_property(n, v))
            .map(|(n, v)| (n.clone(), v.get_type()))
            .collect::<Vec<_>>()
    }));
    let is_const = move || node.instance.with(|i| i.model.type_name == "const");
    view! {
        <div class="node-props" on:pointerdown=stop_on_controls>
            <For each=move || names.get() key=|(n, t)| (n.clone(), data_type_name(t)) let:prop>
                {
                    let (name, value_type) = prop;
                    let label_name = name.clone();
                    let label = Signal::derive(move || property_label(&label_name));
                    let read_name = name.clone();
                    let value = Signal::derive(move || property_value(node, &read_name));
                    let send_name = name.clone();
                    let send = Callback::new(move |value: DataValue| {
                        ed.send(ClientMessage::SetProperty { id, name: send_name.clone(), value });
                    });
                    let text_of = move || match value.get() {
                        Some(DataValue::String(s)) => s,
                        _ => String::new(),
                    };
                    let control = if name.ends_with("_type") {
                        view! {
                            <InlineSelect
                                options=type_options(ed, &TYPE_OPTIONS)
                                selected=Signal::derive(text_of)
                                title=label
                                on_select=Callback::new(move |v| send.run(DataValue::String(v)))
                            />
                        }
                        .into_any()
                    } else if value_type == DataType::Bool {
                        view! {
                            <Switch
                                small=true
                                checked=Signal::derive(move || matches!(value.get(), Some(DataValue::Bool(true))))
                                on_toggle=Callback::new(move |b| send.run(DataValue::Bool(b)))
                            />
                        }
                        .into_any()
                    } else {
                        let type_select = (name == "value" && is_const()).then(|| view! {
                            <InlineSelect
                                options=CONST_TYPES.iter().map(|o| (o.to_string(), i18n::type_name_label(o))).collect()
                                selected=Signal::derive(move || value.get().map(|v| data_type_name(&v.get_type())).unwrap_or_default())
                                title=Signal::derive(move || t!("inline-value-type-title"))
                                on_select=Callback::new(move |t: String| send.run(default_value(&t)))
                            />
                        });
                        view! {
                            {type_select}
                            <InlineValue
                                value=value
                                value_type=value_type
                                title=label
                                on_commit=Callback::new(move |v: Option<DataValue>| {
                                    if let Some(v) = v {
                                        send.run(v);
                                    }
                                })
                            />
                        }
                        .into_any()
                    };
                    view! {
                        <div class="prop-row">
                            <span class="ellipsis prop-label" title=move || label.get()>{move || label.get()}</span>
                            {control}
                        </div>
                    }
                }
            </For>
        </div>
    }
    .into_any()
}

/// Значення непід'єднаного входу даних (ставиться в рядку піна).
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
mod tests {
    use super::*;

    #[test]
    fn typed_parsing() {
        assert_eq!(parse_text("2,5", &DataType::Number), Ok(Some(DataValue::Number(NumberValue::Float(2.5)))));
        assert!(parse_text("abc", &DataType::Number).is_err());
        assert_eq!(parse_text("", &DataType::Number), Ok(None));
        assert_eq!(parse_text("", &DataType::String), Ok(Some(DataValue::String(String::new()))));
        assert_eq!(parse_text("7", &DataType::Any), Ok(Some(DataValue::Number(NumberValue::Int(7)))));
        assert_eq!(parse_text("\"7\"", &DataType::Any), Ok(Some(DataValue::String("7".into()))));
        assert_eq!(parse_text("on", &DataType::Any), Ok(Some(DataValue::String("on".into()))));
        assert_eq!(value_text(&DataValue::String("7".into()), &DataType::Any), "\"7\"");
        assert_eq!(value_text(&DataValue::String("7".into()), &DataType::String), "7");
        assert!(matches!(parse_text("07:30", &DataType::Time), Ok(Some(DataValue::Time(_)))));
        assert!(parse_text("7:99", &DataType::Time).is_err());
        assert_eq!(default_value("duration").get_type(), DataType::Duration);
    }
}
