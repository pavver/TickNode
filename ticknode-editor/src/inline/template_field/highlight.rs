//! Reactive syntax highlighting layer and error range formatting for template fields.

use leptos::prelude::*;
use std::ops::Range;
use ticknode_shared::template::{Highlight, TokenClass, highlight};

/// A rendered segment of template text with its CSS class name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub class: &'static str,
    pub is_error: bool,
    pub is_lint: bool,
}

pub fn token_class_name(class: TokenClass) -> &'static str {
    match class {
        TokenClass::Text => "tk-text",
        TokenClass::Delimiter => "tk-delimiter",
        TokenClass::Keyword => "tk-keyword",
        TokenClass::Input => "tk-input",
        TokenClass::Variable => "tk-variable",
        TokenClass::Field => "tk-field",
        TokenClass::Filter => "tk-filter",
        TokenClass::Test => "tk-test",
        TokenClass::Function => "tk-function",
        TokenClass::String => "tk-string",
        TokenClass::Number => "tk-number",
        TokenClass::Operator => "tk-operator",
        TokenClass::Comment => "tk-comment",
    }
}

/// Clamps and aligns raw error byte range to valid char boundaries.
/// Guarantees at least one char; if at end-of-text, marks the last char.
pub fn clamp_error_range(source: &str, raw_range: Range<usize>) -> Option<Range<usize>> {
    if source.is_empty() {
        return None;
    }
    let len = source.len();
    let mut start = raw_range.start.min(len);
    let mut end = raw_range.end.min(len).max(start);

    if start >= len {
        if let Some((idx, ch)) = source.char_indices().last() {
            return Some(idx..idx + ch.len_utf8());
        }
        return None;
    }

    while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
    }
    while end > start && !source.is_char_boundary(end) {
        end -= 1;
    }

    if start == end {
        let ch = source[start..].chars().next()?;
        end = start + ch.len_utf8();
    }

    Some(start..end)
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

/// Reactive component that renders the highlighted `<pre>` overlay.
#[component]
pub fn TemplateHighlightLayer(
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
        let error_range = ticknode_shared::template::validate(&text)
            .err()
            .and_then(|err| err.range.and_then(|r| clamp_error_range(&text, r)));
        let lint_ranges = if error_range.is_none() {
            let typed_inputs =
                crate::inline::template_field::preview::node_preview_inputs(ed, node_id)
                    .into_iter()
                    .map(|(name, _, ty)| (name, ty))
                    .collect::<Vec<_>>();
            let sheet = ed.open_sheet.get();
            let visible_vars = ed.variables.with(|vars| {
                vars.values()
                    .filter(|v| {
                        v.key.scope != ticknode_shared::VariableScope::Sheet
                            || (sheet.is_some() && v.key.sheet == sheet)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            });
            let lints = ed.template_lints(&text, &typed_inputs, &visible_vars);
            lints.into_iter().map(|l| l.range).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        compute_styled_spans(&text, &raw_highlights, error_range, &lint_ranges)
    });

    view! {
        <pre
            id=format!("template-highlight-{}", node_id.0)
            node_ref=pre_ref
            class="template-highlight"
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
