//! Reactive syntax highlighting layer and error range formatting for formula fields.

use leptos::prelude::*;
use std::ops::Range;
use ticknode_shared::formula::{Highlight, TokenClass, highlight};

pub use crate::inline::template_field::highlight::{StyledSpan, clamp_error_range};

pub fn token_class_name(class: TokenClass) -> &'static str {
    match class {
        TokenClass::Text => "tk-text",
        TokenClass::Number => "tk-number",
        TokenClass::Operator => "tk-operator",
        TokenClass::Function => "tk-function",
        TokenClass::Constant => "tk-constant",
        TokenClass::Input => "tk-input",
        TokenClass::Error => "tk-error",
    }
}

/// Splits highlights by `error_range` or `lint_ranges` so that error regions receive the `tk-error` class
/// and lint regions receive the `st-lint` class.
pub fn compute_styled_spans(
    source: &str,
    highlights: &[Highlight],
    error_range: Option<Range<usize>>,
    lint_ranges: &[Range<usize>],
) -> Vec<StyledSpan> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();

    for h in highlights {
        let base_class = token_class_name(h.class);
        let s = h.range.start;
        let e = h.range.end;

        if let Some(ref err) = error_range {
            let err_s = err.start;
            let err_e = err.end;

            // Part before error
            let pre_e = s.max(err_s).min(e);
            if s < pre_e {
                spans.push(StyledSpan {
                    text: source[s..pre_e].to_string(),
                    class: base_class,
                    is_error: false,
                    is_lint: false,
                });
            }

            // Part inside error
            let in_s = s.max(err_s);
            let in_e = e.min(err_e);
            if in_s < in_e {
                spans.push(StyledSpan {
                    text: source[in_s..in_e].to_string(),
                    class: base_class,
                    is_error: true,
                    is_lint: false,
                });
            }

            // Part after error
            let post_s = s.max(err_e).min(e);
            if post_s < e {
                spans.push(StyledSpan {
                    text: source[post_s..e].to_string(),
                    class: base_class,
                    is_error: false,
                    is_lint: false,
                });
            }
        } else if !lint_ranges.is_empty() {
            let mut points = vec![s, e];
            for lr in lint_ranges {
                if lr.start > s && lr.start < e {
                    points.push(lr.start);
                }
                if lr.end > s && lr.end < e {
                    points.push(lr.end);
                }
            }
            points.sort_unstable();
            points.dedup();
            for w in points.windows(2) {
                let sub_s = w[0];
                let sub_e = w[1];
                let is_lint = lint_ranges
                    .iter()
                    .any(|lr| lr.start <= sub_s && sub_e <= lr.end);
                spans.push(StyledSpan {
                    text: source[sub_s..sub_e].to_string(),
                    class: base_class,
                    is_error: false,
                    is_lint,
                });
            }
        } else {
            spans.push(StyledSpan {
                text: source[s..e].to_string(),
                class: base_class,
                is_error: false,
                is_lint: false,
            });
        }
    }

    spans
}

/// Reactive component that renders the highlighted `<pre>` overlay for formulas.
#[component]
pub fn FormulaHighlightLayer(
    node_id: ticknode_shared::NodeId,
    pre_ref: NodeRef<leptos::html::Pre>,
    draft: Signal<String>,
    inputs: Signal<Vec<String>>,
) -> impl IntoView {
    let ed = crate::state::use_editor();
    let spans = Memo::new(move |_| {
        let text = draft.get();
        let inps = inputs.get();
        let raw_highlights = highlight(&text, &inps);
        let error_range = ticknode_shared::formula::validate(&text)
            .err()
            .and_then(|err| clamp_error_range(&text, err.range));
        let lint_ranges = if error_range.is_none() {
            let typed_inputs =
                crate::inline::template_field::preview::node_preview_inputs(ed, node_id)
                    .into_iter()
                    .map(|(name, _, ty)| (name, ty))
                    .collect::<Vec<_>>();
            let lints = ticknode_shared::formula::lint(&text, &typed_inputs);
            lints.into_iter().map(|l| l.range).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        compute_styled_spans(&text, &raw_highlights, error_range, &lint_ranges)
    });

    view! {
        <pre
            id=format!("formula-highlight-{}", node_id.0)
            node_ref=pre_ref
            class="formula-highlight"
            aria-hidden="true"
        >
            {move || {
                spans.get().into_iter().map(|span| {
                    let mut class_name = span.class.to_string();
                    if span.is_error {
                        class_name.push_str(" tk-error");
                    }
                    if span.is_lint {
                        class_name.push_str(" st-lint");
                    }
                    view! { <span class=class_name>{span.text}</span> }
                }).collect_view()
            }}
            "\n"
        </pre>
    }
}
