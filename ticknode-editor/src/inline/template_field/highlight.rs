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

/// Splits highlights by `error_range` so that error regions receive the `tk-error` class.
pub fn compute_styled_spans(
    source: &str,
    highlights: &[Highlight],
    error_range: Option<Range<usize>>,
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
                });
            }

            // Part after error
            let post_s = s.max(err_e).min(e);
            if post_s < e {
                spans.push(StyledSpan {
                    text: source[post_s..e].to_string(),
                    class: base_class,
                    is_error: false,
                });
            }
        } else {
            spans.push(StyledSpan {
                text: source[s..e].to_string(),
                class: base_class,
                is_error: false,
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
    let spans = Memo::new(move |_| {
        let text = draft.get();
        let inps = inputs.get();
        let raw_highlights = highlight(&text, &inps);
        let error_range = ticknode_shared::template::validate(&text)
            .err()
            .and_then(|err| err.range.and_then(|r| clamp_error_range(&text, r)));
        compute_styled_spans(&text, &raw_highlights, error_range)
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
                    let class_name = if span.is_error {
                        format!("{} tk-error", span.class)
                    } else {
                        span.class.to_string()
                    };
                    view! { <span class=class_name>{span.text}</span> }
                }).collect_view()
            }}
            "\n"
        </pre>
    }
}
