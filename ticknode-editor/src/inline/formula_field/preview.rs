//! Live result preview, validation error display, and sample value integration for formulas.

use crate::i18n::{t, ui_text};
use crate::inline::formula_field::FormulaDraft;
use crate::inline::template_field::preview::{is_sample_value_used, node_preview_inputs};
#[cfg(target_arch = "wasm32")]
use crate::inline::template_field::samples::TemplateSamplesState;
use crate::state::use_editor;
use leptos::prelude::*;
use std::collections::HashMap;
use ticknode_shared::formula::{FormulaValue, compile, infer_type, lint, validate};
use ticknode_shared::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaPreviewStatus {
    Empty,
    SyntaxError(String),
    Warning(String),
    Waiting,
    EvalError(String),
    Result(String),
}

/// Reactive status row for a formula node.
#[component]
pub fn FormulaStatusRow(node: NodeId) -> impl IntoView {
    let ed = use_editor();
    let draft_signal = use_context::<FormulaDraft>().map(|d| d.0);

    let status = Memo::new(move |_| {
        let text = draft_signal.map(|d| d.get()).unwrap_or_default();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return FormulaPreviewStatus::Empty;
        }

        // 1. Syntax check
        if let Err(err) = validate(trimmed) {
            return FormulaPreviewStatus::SyntaxError(ui_text(&err.message));
        }

        let inputs = node_preview_inputs(ed, node);
        let typed_inputs: Vec<(String, DataType)> = inputs
            .iter()
            .map(|(n, _, ty)| (n.clone(), ty.clone()))
            .collect();

        // 2. Lint check (unknown function with suggestion, arity, quantity mismatches)
        let lints = lint(trimmed, &typed_inputs);
        if let Some(first_lint) = lints.first() {
            return FormulaPreviewStatus::Warning(ui_text(&first_lint.message));
        }

        // 3. Compile and evaluate
        let Ok(compiled) = compile(trimmed) else {
            return FormulaPreviewStatus::SyntaxError(t!("error-formula-syntax"));
        };

        let mut values_map = HashMap::new();
        for (name, val, _) in &inputs {
            if let Some(v) = val {
                values_map.insert(name.as_str(), v);
            }
        }

        for var in &compiled.variables {
            if !values_map.contains_key(var.as_str()) {
                return FormulaPreviewStatus::Waiting;
            }
        }

        let lookup = |name: &str| -> Option<FormulaValue> {
            match values_map.get(name)? {
                DataValue::Number(n) => Some(FormulaValue::Number(*n)),
                DataValue::Bool(b) => Some(FormulaValue::Bool(*b)),
                _ => None,
            }
        };

        match compiled.eval(&lookup) {
            Ok(res) => {
                let input_types_map: HashMap<String, DataType> = typed_inputs.into_iter().collect();
                let inferred_ty =
                    infer_type(&compiled.ast, &input_types_map).unwrap_or(DataType::Number);
                let ty_label = ed.type_title(&inferred_ty);
                let value = match res {
                    FormulaValue::Number(n) => DataValue::Number(n),
                    FormulaValue::Bool(b) => DataValue::Bool(b),
                };
                let val_str = crate::i18n::value_as_text(&value, &inferred_ty);
                FormulaPreviewStatus::Result(format!("{val_str} ({ty_label})"))
            }
            Err(e) => FormulaPreviewStatus::EvalError(ui_text(&e.message)),
        }
    });

    let has_samples = Memo::new(move |_| {
        let Some(nd) = ed.node_tracked(node) else {
            return false;
        };
        let pins = nd.instance.with(|i| {
            i.definition
                .pins
                .iter()
                .filter(|p| {
                    p.direction == PinDirection::Input && matches!(p.pin_type, PinType::Data(_))
                })
                .map(|p| (p.id, p.name.clone()))
                .collect::<Vec<_>>()
        });

        ed.sample_values.with(|samples| {
            pins.iter().any(|(pin_id, name)| {
                let own_trace = nd.trace.with(|t| {
                    t.as_ref()
                        .and_then(|tr| {
                            crate::node_status::trace_value(tr, *pin_id, PinDirection::Input)
                        })
                        .cloned()
                });
                let source_trace = if own_trace.is_none() {
                    let src = ed.data.with(|d| {
                        d.iter()
                            .find(|c| c.dst_node == node && c.dst_pin == *pin_id)
                            .map(|c| (c.src_node, c.src_pin))
                    });
                    src.and_then(|(sn, sp)| {
                        ed.node_tracked(sn).and_then(|n| {
                            n.trace.with(|tr| {
                                tr.as_ref()
                                    .and_then(|t| {
                                        crate::node_status::trace_value(t, sp, PinDirection::Output)
                                    })
                                    .cloned()
                            })
                        })
                    })
                } else {
                    None
                };
                let inline_val = if own_trace.is_none() && source_trace.is_none() {
                    let prop = ticknode_shared::input_value_property(*pin_id);
                    nd.instance.with(|i| {
                        i.model
                            .properties
                            .iter()
                            .find(|(n, _)| *n == prop)
                            .map(|(_, v)| v.clone())
                    })
                } else {
                    None
                };
                let sample_val = samples.get(&(node, name.clone()));
                is_sample_value_used(
                    own_trace.as_ref(),
                    source_trace.as_ref(),
                    inline_val.as_ref(),
                    sample_val,
                )
            })
        })
    });

    let toggle_samples = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        #[cfg(target_arch = "wasm32")]
        if let Some(target) = ev.current_target()
            && let Ok(el) = target.dyn_into::<web_sys::Element>()
        {
            let rect = el.get_bounding_client_rect();
            let anchor = (rect.left(), rect.top(), rect.width(), rect.height());
            ed.template_samples.update(|cur| {
                if cur.as_ref().is_some_and(|s| s.node == node) {
                    *cur = None;
                } else {
                    *cur = Some(TemplateSamplesState { node, anchor });
                }
            });
        }
    };

    let status_class = move || match status.get() {
        FormulaPreviewStatus::SyntaxError(_) | FormulaPreviewStatus::EvalError(_) => {
            "template-status error"
        }
        FormulaPreviewStatus::Warning(_) => "template-status warning",
        _ => "template-status preview",
    };

    view! {
        <div class=status_class>
            {move || match status.get() {
                FormulaPreviewStatus::SyntaxError(msg) => {
                    let title = msg.clone();
                    view! {
                        <span class="mdi mdi-alert-circle-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" title=title>
                            {msg}
                        </span>
                    }.into_any()
                }
                FormulaPreviewStatus::Warning(msg) => {
                    let title = msg.clone();
                    view! {
                        <span class="mdi mdi-alert-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" title=title>
                            {msg}
                        </span>
                    }.into_any()
                }
                FormulaPreviewStatus::EvalError(msg) => {
                    let title = msg.clone();
                    view! {
                        <span class="mdi mdi-alert-circle-outline template-status-icon" aria-hidden="true"></span>
                        <span class="template-status-text ellipsis" title=title>
                            {msg}
                        </span>
                    }.into_any()
                }
                FormulaPreviewStatus::Waiting => {
                    let wait_text = t!("formula-preview-waiting");
                    let title = wait_text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        {move || has_samples.get().then(|| view! {
                            <span
                                class="template-preview-sample-badge"
                                title=t!("formula-preview-samples")
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:click=toggle_samples
                            >
                                {t!("formula-preview-sample-badge")}
                            </span>
                        })}
                        <span class="template-status-text ellipsis" title=title>
                            {wait_text}
                        </span>
                        <button
                            type="button"
                            class="template-preview-samples template-preview-samples-btn"
                            title=t!("formula-preview-samples")
                            on:pointerdown=|ev| ev.stop_propagation()
                            on:click=toggle_samples
                        >
                            {t!("formula-preview-samples")}
                        </button>
                    }.into_any()
                }
                FormulaPreviewStatus::Empty => {
                    let empty_text = t!("formula-preview-empty");
                    let title = empty_text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        {move || has_samples.get().then(|| view! {
                            <span
                                class="template-preview-sample-badge"
                                title=t!("formula-preview-samples")
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:click=toggle_samples
                            >
                                {t!("formula-preview-sample-badge")}
                            </span>
                        })}
                        <span class="template-status-text ellipsis" style="font-style: italic;" title=title>
                            {empty_text}
                        </span>
                    }.into_any()
                }
                FormulaPreviewStatus::Result(text) => {
                    let title = text.clone();
                    view! {
                        <span class="mdi mdi-eye-outline template-status-icon" aria-hidden="true"></span>
                        {move || has_samples.get().then(|| view! {
                            <span
                                class="template-preview-sample-badge"
                                title=t!("formula-preview-samples")
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:click=toggle_samples
                            >
                                {t!("formula-preview-sample-badge")}
                            </span>
                        })}
                        <span class="template-status-text ellipsis" title=title>
                            {text}
                        </span>
                        <button
                            type="button"
                            class="template-preview-samples template-preview-samples-btn"
                            title=t!("formula-preview-samples")
                            on:pointerdown=|ev| ev.stop_propagation()
                            on:click=toggle_samples
                        >
                            {t!("formula-preview-samples")}
                        </button>
                    }.into_any()
                }
            }}
        </div>
    }
}
