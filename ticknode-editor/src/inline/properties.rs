//! Node property rows, including type selectors and dynamic pickers.

use super::*;
use crate::geometry::is_inline_property;
use crate::match_editor::stop_on_controls;
use crate::panels::property_label;
use crate::picker::PickerField;
use crate::ui::Switch;

/// Selectable types for `*_type` properties (list elements, object field); custom types are appended from the registry.
const TYPE_OPTIONS: [&str; 8] = ["number", "string", "bool", "time", "datetime", "duration", "color", "object"];
/// Selectable value types for a Constant node.
const CONST_TYPES: [&str; 7] = ["number", "string", "bool", "time", "datetime", "duration", "color"];

pub(super) fn default_value(type_name: &str) -> DataValue {
    match parse_data_type(type_name) {
        Some(DataType::Bool) => DataValue::Bool(false),
        Some(t) => text::default_of(&t),
        None => DataValue::Number(NumberValue::Int(0)),
    }
}

/// `(value, label)` pairs: built-in types followed by custom plugin types (labeled with `title`).
fn type_options(ed: Editor, builtin: &[&str]) -> Vec<(String, String)> {
    let mut options: Vec<_> = builtin.iter().map(|o| (o.to_string(), i18n::type_name_label(o))).collect();
    ed.types.with_untracked(|types| {
        options.extend(types.definitions().into_iter().map(|d| (d.name.clone(), format!("{} ({})", i18n::type_def_title(&d), d.name))));
    });
    options
}

/// Compact dropdown select on a node.
#[component]
fn InlineSelect(
    mut options: Vec<(String, String)>,
    #[prop(into)] selected: Signal<String>,
    on_select: Callback<String>,
    #[prop(optional, into)] title: Signal<String>,
) -> impl IntoView {
    // A value not present in the options list (from a missing plugin's type) remains visible
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

/// Property rows between the node header and pins (see `geometry::is_inline_property`):
/// types use a dropdown, booleans use a switch, others use an input field. On a Constant node,
/// a type selector is shown next to the value.
#[component]
pub fn NodeProps(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else { return ().into_any() };
    let names = Memo::new(move |_| node.instance.with(|i| {
        i.model.properties.iter()
            .filter(|(n, v)| is_inline_property(n, v))
            .map(|(n, v)| {
                // Only a string property can be shown as a drop-down or picker
                let described = matches!(v, DataValue::String(_))
                    .then(|| i.definition.properties.iter().find(|p| p.name == *n).cloned())
                    .flatten()
                    .filter(|p| p.kind != PropertyKind::Text || !p.title.is_empty());
                (n.clone(), v.get_type(), described)
            })
            .collect::<Vec<_>>()
    }));
    let type_name = move || node.instance.with(|i| i.model.type_name.clone());
    let is_const = move || node.instance.with(|i| i.model.type_name == "const");
    view! {
        <div class="node-props" on:pointerdown=stop_on_controls>
            <For each=move || names.get() key=|(n, t, d)| (n.clone(), data_type_name(t), format!("{d:?}")) let:prop>
                {
                    let (name, value_type, described) = prop;
                    let label_name = name.clone();
                    let label_def = described.clone();
                    let label = Signal::derive(move || match &label_def {
                        Some(def) => i18n::property_title(&type_name(), def),
                        None => property_label(&label_name),
                    });
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
                    let control = if let Some(PropertyKind::Select(options)) = described.as_ref().map(|d| d.kind.clone()) {
                        let owner = type_name();
                        let options = options.into_iter()
                            .map(|(v, l)| { let label = i18n::property_option_label(&owner, &name, &v, &l); (v, label) })
                            .collect();
                        view! {
                            <InlineSelect
                                options=options
                                selected=Signal::derive(text_of)
                                title=label
                                on_select=Callback::new(move |v| send.run(DataValue::String(v)))
                            />
                        }
                        .into_any()
                    } else if let Some(PropertyKind::Dynamic { source, plugin }) = described.as_ref().map(|d| d.kind.clone()) {
                        view! {
                            <PickerField
                                node=id
                                property=name.clone()
                                plugin=plugin
                                source=source
                                value=Signal::derive(text_of)
                                title=label
                            />
                        }
                        .into_any()
                    } else if name.ends_with("_type") {
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
