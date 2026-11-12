//! Live preview rendering and input collection for the template status row.

use std::sync::Arc;
use leptos::prelude::*;
use ticknode_shared::*;
use crate::i18n::t;
use crate::inline::template_field::TemplateDraft;
use crate::state::Editor;

/// Resolves the effective input value according to precedence:
/// own trace input > connected source trace output > inline value property.
pub fn resolve_input_value(
    own_trace: Option<&DataValue>,
    source_trace: Option<&DataValue>,
    inline_value: Option<&DataValue>,
) -> Option<DataValue> {
    own_trace
        .or(source_trace)
        .or(inline_value)
        .cloned()
}

/// Plain descriptor for unit testing input collection precedence.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct PinPreviewInputSource<'a> {
    pub pin_id: PinId,
    pub name: &'a str,
    pub declared_type: DataType,
    pub own_trace: Option<&'a DataValue>,
    pub source_trace: Option<&'a DataValue>,
    pub inline_value: Option<&'a DataValue>,
}

/// Pure helper collecting template input tuples from plain descriptors.
#[allow(dead_code)]
pub fn collect_preview_inputs(
    sources: &[PinPreviewInputSource<'_>],
) -> Vec<(String, Option<DataValue>, DataType)> {
    sources
        .iter()
        .map(|s| {
            let val = resolve_input_value(s.own_trace, s.source_trace, s.inline_value);
            (s.name.to_string(), val, s.declared_type.clone())
        })
        .collect()
}

/// Collects input data pin values from live editor signals in pin order.
pub fn node_preview_inputs(ed: Editor, id: NodeId) -> Vec<(String, Option<DataValue>, DataType)> {
    let Some(node) = ed.node_tracked(id) else { return Vec::new() };
    let pins: Vec<(PinId, String, DataType)> = node.instance.with(|i| {
        i.definition.pins.iter()
            .filter(|p| p.direction == PinDirection::Input && matches!(p.pin_type, PinType::Data(_)))
            .map(|p| {
                let ty = match &p.pin_type { PinType::Data(t) => t.clone(), _ => DataType::Any };
                (p.id, p.name.clone(), ty)
            })
            .collect()
    });

    pins.into_iter().map(|(pin_id, name, default_ty)| {
        let declared_ty = ed.source_type(id, pin_id).unwrap_or(default_ty);

        let own_trace = node.trace.with(|t| {
            t.as_ref().and_then(|tr| crate::node_status::trace_value(tr, pin_id, PinDirection::Input)).cloned()
        });

        let source_trace = if own_trace.is_none() {
            let src = ed.data.with(|d| {
                d.iter().find(|c| c.dst_node == id && c.dst_pin == pin_id).map(|c| (c.src_node, c.src_pin))
            });
            src.and_then(|(src_node, src_pin)| {
                ed.node_tracked(src_node).and_then(|sn| {
                    sn.trace.with(|tr| {
                        tr.as_ref().and_then(|t| crate::node_status::trace_value(t, src_pin, PinDirection::Output)).cloned()
                    })
                })
            })
        } else {
            None
        };

        let inline_value = if own_trace.is_none() && source_trace.is_none() {
            let prop_name = ticknode_shared::input_value_property(pin_id);
            node.instance.with(|i| {
                i.model.properties.iter().find(|(n, _)| *n == prop_name).map(|(_, v)| v.clone())
            })
        } else {
            None
        };

        let value = resolve_input_value(own_trace.as_ref(), source_trace.as_ref(), inline_value.as_ref());
        (name, value, declared_ty)
    }).collect()
}

/// Status row evaluation state.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewStatus {
    SyntaxError { line: usize, column: usize, message: String },
    RenderError(String),
    Waiting,
    Empty,
    Text(String),
}

/// Pure evaluation of current template draft against collected inputs.
pub fn evaluate_preview(
    draft: &str,
    inputs: &[(String, Option<DataValue>, DataType)],
) -> PreviewStatus {
    if let Err(err) = ticknode_shared::template::validate(draft) {
        return PreviewStatus::SyntaxError {
            line: err.line.unwrap_or(1),
            column: err.column.unwrap_or(1),
            message: err.message,
        };
    }

    if !inputs.is_empty() && inputs.iter().all(|(_, val, _)| val.is_none()) {
        return PreviewStatus::Waiting;
    }

    let compiled = match ticknode_shared::template::CompiledTemplate::new(draft) {
        Ok(c) => c,
        Err(err) => return PreviewStatus::RenderError(err.message),
    };

    let format = |v: &DataValue, t: &DataType| crate::i18n::value_as_text(v, t);
    let runtime_text: Arc<dyn Fn(&DataValue) -> String + Send + Sync> =
        Arc::new(|v| crate::i18n::value_as_text(v, &DataType::Any));

    match compiled.render(inputs, &format, runtime_text) {
        Ok(text) if text.is_empty() => PreviewStatus::Empty,
        Ok(text) => PreviewStatus::Text(text),
        // A missing input usually explains the failure (`msg.chat` on an undefined `msg`): wait for values.
        Err(_) if inputs.iter().any(|(_, value, _)| value.is_none()) => PreviewStatus::Waiting,
        Err(err) => PreviewStatus::RenderError(err.message),
    }
}

/// Fixed-height status row rendered directly under the `template` property.
#[component]
pub fn TemplateStatusRow(node: NodeId) -> impl IntoView {
    let ed = crate::state::use_editor();
    let draft = use_context::<TemplateDraft>().map(|d| d.0).unwrap_or_else(|| {
        let stored = ed.node_tracked(node).and_then(|n| {
            n.instance.with(|i| {
                i.model.properties.iter().find(|(name, _)| name == "template").and_then(|(_, v)| match v {
                    DataValue::String(s) => Some(s.clone()),
                    _ => None,
                })
            })
        }).unwrap_or_default();
        RwSignal::new(stored)
    });

    let inputs = Memo::new(move |_| node_preview_inputs(ed, node));
    let preview = Memo::new(move |_| {
        let text = draft.get();
        let ins = inputs.get();
        evaluate_preview(&text, &ins)
    });

    view! {
        <div
            class=move || match preview.get() {
                PreviewStatus::SyntaxError { .. } => "prop-row template-status error",
                PreviewStatus::RenderError(_) => "prop-row template-status warning",
                PreviewStatus::Waiting => "prop-row template-status preview waiting",
                PreviewStatus::Empty | PreviewStatus::Text(_) => "prop-row template-status preview",
            }
            style=format!("height:{}px", crate::geometry::ROW_H)
        >
            {move || match preview.get() {
                PreviewStatus::SyntaxError { line, column, message } => {
                    let err_text = t!("template-error", line = line, column = column, message = message.clone());
                    let title = err_text.clone();
                    view! {
                        <span class="template-status-text ellipsis" title=title>
                            {err_text}
                        </span>
                    }.into_any()
                }
                PreviewStatus::RenderError(msg) => {
                    let err_text = t!("template-render-error", message = msg.clone());
                    let title = err_text.clone();
                    view! {
                        <span class="template-status-text ellipsis" title=title>
                            {err_text}
                        </span>
                    }.into_any()
                }
                PreviewStatus::Waiting => {
                    let wait_text = t!("template-preview-waiting");
                    let title = wait_text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" title=title>
                            {wait_text}
                        </span>
                    }.into_any()
                }
                PreviewStatus::Empty => {
                    let empty_text = t!("template-preview-empty");
                    let title = empty_text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" style="font-style: italic;" title=title>
                            {empty_text}
                        </span>
                    }.into_any()
                }
                PreviewStatus::Text(text) => {
                    let title = text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" title=title>
                            {text}
                        </span>
                    }.into_any()
                }
            }}
        </div>
    }
}
