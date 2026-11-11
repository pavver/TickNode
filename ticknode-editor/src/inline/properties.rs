//! Node property rows, including type selectors and dynamic pickers.

use super::*;
use crate::geometry::visible_node_property;
use crate::match_editor::stop_on_controls;
use crate::panels::property_label;
use crate::picker::PickerField;
use crate::ui::Switch;

/// Selectable types for `*_type` properties (list elements, object field); custom types are appended from the registry.
pub(crate) const TYPE_OPTIONS: [&str; 8] = ["number", "string", "bool", "time", "datetime", "duration", "color", "object"];
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
pub(crate) fn type_options(ed: Editor, builtin: &[&str]) -> Vec<(String, String)> {
    let mut options: Vec<_> = builtin.iter().map(|o| {
        let label = match parse_data_type(o) {
            Some(t @ DataType::List(_)) => i18n::data_type_title(&TypeRegistry::new(), &t),
            _ => i18n::type_name_label(o),
        };
        (o.to_string(), label)
    }).collect();
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
    #[prop(optional)] localization: Option<(String, String)>,
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
            {options.into_iter().map(|(value, label)| {
                let context = localization.clone();
                let key = value.clone();
                view! { <option value=value>{move || match &context {
                    Some((owner, name)) => i18n::property_option_label(owner, name, &key, &label),
                    None => label.clone(),
                }}</option> }
            }).collect_view()}
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
            .filter(|(n, v)| visible_node_property(&i.model.type_name, n, v, &i.model.properties))
            .map(|(n, v)| {
                // Only a string property can be shown as a drop-down or picker
                let described = matches!(v, DataValue::String(_))
                    .then(|| i.definition.properties.iter().find(|p| p.name == *n).cloned())
                    .flatten()
                    .filter(|p| p.kind != PropertyKind::Text || !p.title.is_empty());
                let declared = if i.model.type_name == "const" && n == "value" {
                    i.definition.pins.iter().find_map(|p| match &p.pin_type { PinType::Data(t) if p.id == PinId(0) => Some(t.clone()), _ => None }).unwrap_or_else(|| v.get_type())
                } else { v.get_type() };
                (n.clone(), declared, described)
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
                    let height_name = name.clone();
                    let row_height = Signal::derive(move || ed.layout(id).map_or(crate::geometry::ROW_H, |l| l.property_height(&height_name)));
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
                        view! {
                            <InlineSelect
                                options=options
                                localization=(owner, name.clone())
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
                                options=if type_name() == "text/to_text" { type_options(ed, &["any", "number", "string", "bool", "time", "datetime", "duration", "color", "object", "list<any>"]) } else { type_options(ed, &TYPE_OPTIONS) }
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
                                options=type_options(ed, &CONST_TYPES).into_iter().filter(|(v, _)| CONST_TYPES.contains(&v.as_str()) || ticknode_shared::quantity::kind(v).is_some()).collect()
                                selected=Signal::derive(move || node.instance.with(|i| i.definition.pins.iter().find_map(|p| match &p.pin_type {
                                    PinType::Data(t) if p.id == PinId(0) => Some(data_type_name(t)), _ => None,
                                }).unwrap_or_default()))
                                title=Signal::derive(move || t!("inline-value-type-title"))
                                on_select=Callback::new(move |t: String| ed.send(ClientMessage::Batch(vec![
                                    ClientMessage::SetProperty { id, name: "value_type".into(), value: t.clone().into() },
                                    ClientMessage::SetProperty { id, name: "value".into(), value: default_value(&t) },
                                ])))
                            />
                        });
                        view! {
                            {type_select}
                            <InlineValue
                                value=value
                                value_type=value_type
                                multiline=Signal::derive(move || row_height.get() > crate::geometry::ROW_H)
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
                        <div class="prop-row" class:grown={move || row_height.get() > crate::geometry::ROW_H} style=move || format!("height:{}px", row_height.get())>
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
