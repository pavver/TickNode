//! Popup list for trying sample input values in template preview.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::t;
use crate::inline::InlineValue;
use crate::picker::{placement, viewport};
use crate::state::{Editor, use_editor};

/// Open template preview sample values popup state: anchor rect in client coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSamplesState {
    pub node: NodeId,
    pub anchor: (f64, f64, f64, f64),
}

/// Plain descriptor for an input awaiting a value for template preview.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingInput {
    pub pin_id: PinId,
    pub name: String,
    pub declared_type: DataType,
}

/// Collects inputs that have neither an execution trace value nor an inline value.
pub fn node_waiting_inputs(ed: Editor, id: NodeId) -> Vec<WaitingInput> {
    let Some(node) = ed.node_tracked(id) else {
        return Vec::new();
    };
    let pins: Vec<(PinId, String, DataType)> = node.instance.with(|i| {
        i.definition
            .pins
            .iter()
            .filter(|p| {
                p.direction == PinDirection::Input && matches!(p.pin_type, PinType::Data(_))
            })
            .map(|p| {
                let ty = match &p.pin_type {
                    PinType::Data(t) => t.clone(),
                    _ => DataType::Any,
                };
                (p.id, p.name.clone(), ty)
            })
            .collect()
    });

    pins.into_iter()
        .filter_map(|(pin_id, name, default_ty)| {
            let declared_ty = ed.source_type(id, pin_id).unwrap_or(default_ty);

            let has_own_trace = node.trace.with(|t| {
                t.as_ref()
                    .and_then(|tr| crate::node_status::trace_value(tr, pin_id, PinDirection::Input))
                    .is_some()
            });
            if has_own_trace {
                return None;
            }

            let has_source_trace = {
                let src = ed.data.with(|d| {
                    d.iter()
                        .find(|c| c.dst_node == id && c.dst_pin == pin_id)
                        .map(|c| (c.src_node, c.src_pin))
                });
                src.and_then(|(src_node, src_pin)| {
                    ed.node_tracked(src_node).and_then(|sn| {
                        sn.trace.with(|tr| {
                            tr.as_ref()
                                .and_then(|t| {
                                    crate::node_status::trace_value(
                                        t,
                                        src_pin,
                                        PinDirection::Output,
                                    )
                                })
                                .map(|_| ())
                        })
                    })
                })
                .is_some()
            };
            if has_source_trace {
                return None;
            }

            let has_inline = {
                let prop_name = ticknode_shared::input_value_property(pin_id);
                node.instance
                    .with(|i| i.model.properties.iter().any(|(n, _)| *n == prop_name))
            };
            if has_inline {
                return None;
            }

            Some(WaitingInput {
                pin_id,
                name,
                declared_type: declared_ty,
            })
        })
        .collect()
}

/// Returns true if any waiting input of `id` is currently populated by a sample value.
pub fn node_has_active_samples(ed: Editor, id: NodeId) -> bool {
    let waiting = node_waiting_inputs(ed, id);
    ed.sample_values.with(|s| {
        waiting
            .iter()
            .any(|w| s.contains_key(&(id, w.name.clone())))
    })
}

/// Global layer hosting the active template sample values popup.
#[component]
pub fn TemplateSamplesLayer() -> impl IntoView {
    let ed = use_editor();

    // Close on camera pan / zoom
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() {
            ed.template_samples.set(None);
        }
    });

    // Close if the anchored node is removed
    Effect::new(move |_| {
        if let Some(state) = ed.template_samples.get()
            && ed.node_tracked(state.node).is_none()
        {
            ed.template_samples.set(None);
        }
    });

    view! {
        {move || ed.template_samples
            .get()
            .map(|state| view! { <TemplateSamplesPopup state=state /> })}
    }
}

#[component]
fn TemplateSamplesPopup(state: TemplateSamplesState) -> impl IntoView {
    let ed = use_editor();
    let node_id = state.node;

    let waiting = Memo::new(move |_| node_waiting_inputs(ed, node_id));
    let has_any_samples = Memo::new(move |_| {
        ed.sample_values
            .with(|s| s.keys().any(|(n, _)| *n == node_id))
    });

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            ed.template_samples.set(None);
        }
    };

    let place = placement(state.anchor, viewport());
    let vertical = if place.up {
        format!("bottom:{}px", place.edge)
    } else {
        format!("top:{}px", place.edge)
    };
    let style = format!(
        "left:{}px;width:{}px;{vertical};max-height:{}px",
        place.left, place.width, place.max_height
    );

    view! {
        <div
            class="picker-popup template-samples-popup"
            style=style
            tabindex="-1"
            on:pointerdown=|ev| ev.stop_propagation()
            on:keydown=on_keydown
        >
            <div class="template-samples-header">
                <span class="template-samples-title">{t!("template-preview-samples-title")}</span>
                {move || has_any_samples.get().then(|| {
                    let on_clear = move |_| ed.clear_sample_values(node_id);
                    view! {
                        <button
                            type="button"
                            class="template-samples-clear"
                            on:click=on_clear
                        >
                            {t!("template-preview-samples-clear")}
                        </button>
                    }
                })}
            </div>
            <div class="template-samples-list">
                {move || {
                    let items = waiting.get();
                    if items.is_empty() {
                        view! {
                            <div class="picker-note">
                                {t!("template-preview-no-waiting-inputs")}
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            {items.into_iter().map(|item| {
                                view! { <SampleInputRow node=node_id item=item /> }
                            }).collect_view()}
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn SampleInputRow(node: NodeId, item: WaitingInput) -> impl IntoView {
    let ed = use_editor();
    let name = item.name.clone();
    let name_for_commit = item.name.clone();
    let value_type = item.declared_type;

    let sample_val = Signal::derive(move || {
        ed.sample_values
            .with(|s| s.get(&(node, name.clone())).cloned())
    });

    let on_commit = Callback::new(move |val: Option<DataValue>| match val {
        Some(v) => ed.sample_values.update(|s| {
            s.insert((node, name_for_commit.clone()), v);
        }),
        None => ed.sample_values.update(|s| {
            s.remove(&(node, name_for_commit.clone()));
        }),
    });

    let label = item.name.clone();
    let title = item.name.clone();

    view! {
        <div class="template-sample-row">
            <span class="template-sample-name" title=title>
                {label}
            </span>
            <div class="template-sample-ctrl">
                <InlineValue
                    value=sample_val
                    value_type=value_type
                    on_commit=on_commit
                    optional=true
                />
            </div>
        </div>
    }
}
