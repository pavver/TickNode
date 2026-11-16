//! Hover token hit-testing and tooltip resolution for formula fields.

use crate::state::Editor;
use std::ops::Range;
use ticknode_shared::formula::find_function;
use ticknode_shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaHoverToken {
    pub range: Range<usize>,
    pub name: String,
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'.'
}

/// Finds all identifier token ranges in a formula string.
pub fn find_formula_tokens(source: &str) -> Vec<Range<usize>> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if is_ident_char(bytes[i]) {
            let start = i;
            while i < len && is_ident_char(bytes[i]) {
                i += 1;
            }
            tokens.push(start..i);
        } else if let Some(ch) = source[i..].chars().next() {
            i += ch.len_utf8();
        } else {
            i += 1;
        }
    }
    tokens
}

/// Hit-tests a formula token under the pointer in a textarea.
#[cfg(target_arch = "wasm32")]
pub fn hit_test_token(
    textarea: &web_sys::HtmlTextAreaElement,
    client_x: f64,
    client_y: f64,
    source: &str,
) -> Option<FormulaHoverToken> {
    let ta_rect = textarea.get_bounding_client_rect();
    if client_x < ta_rect.left()
        || client_x > ta_rect.right()
        || client_y < ta_rect.top()
        || client_y > ta_rect.bottom()
    {
        return None;
    }

    let tokens = find_formula_tokens(source);
    if tokens.is_empty() {
        return None;
    }

    let mirror = crate::inline::template_field::caret::Mirror::new(textarea)?;
    let mut last_end = 0;
    let mut spans = Vec::new();
    for token_range in &tokens {
        if token_range.start > last_end {
            let _ = mirror.div.append_child(
                &mirror
                    .document
                    .create_text_node(&source[last_end..token_range.start]),
            );
        }
        if let Ok(span) = mirror.document.create_element("span") {
            span.set_text_content(Some(&source[token_range.clone()]));
            let _ = mirror.div.append_child(&span);
            spans.push((token_range.clone(), span));
        }
        last_end = token_range.end;
    }
    if last_end < source.len() {
        let _ = mirror
            .div
            .append_child(&mirror.document.create_text_node(&source[last_end..]));
    }
    let frame = mirror.frame(textarea);
    let hit = spans.iter().find_map(|(range, span)| {
        let (x, y, w, h) = frame.screen_rect(&span.get_bounding_client_rect());
        (client_x >= x && client_x <= x + w && client_y >= y && client_y <= y + h)
            .then_some(range.clone())
    });
    mirror.remove();

    hit.map(|range| FormulaHoverToken {
        name: source[range.clone()].to_string(),
        range,
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn hit_test_token(
    _textarea: &web_sys::HtmlTextAreaElement,
    _client_x: f64,
    _client_y: f64,
    _source: &str,
) -> Option<FormulaHoverToken> {
    None
}

/// Resolves the hover documentation tooltip text for a formula token.
#[allow(dead_code)]
pub fn resolve_hover_tooltip(ed: Editor, node: NodeId, token_name: &str) -> Option<String> {
    if let Some(doc) = find_function(token_name) {
        let desc_msg = UiMessage::new(format!("formula-fn-{}", doc.name), doc.description);
        let desc = crate::i18n::ui_text(&desc_msg);
        return Some(format!("{}\n{}", doc.signature, desc));
    }

    const CONSTANTS: &[(&str, &str, &str)] = &[
        ("pi", "3.14159...", "Archimedes' constant pi"),
        ("e", "2.71828...", "Euler's number e"),
        ("tau", "6.28318...", "Circle constant tau (2*pi)"),
        ("inf", "Infinity", "Positive infinity"),
        ("nan", "NaN", "Not a number (NaN)"),
        ("true", "Bool", "Boolean true"),
        ("false", "Bool", "Boolean false"),
    ];
    if let Some((_, detail, desc)) = CONSTANTS.iter().find(|(name, _, _)| *name == token_name) {
        let msg = UiMessage::new(format!("formula-const-{token_name}"), *desc);
        let desc_text = crate::i18n::ui_text(&msg);
        return Some(format!("{token_name}: {detail} — {desc_text}"));
    }

    let preview_inputs = crate::inline::template_field::preview::node_preview_inputs(ed, node);
    if let Some((_, _, ty)) = preview_inputs.iter().find(|(n, _, _)| n == token_name) {
        let type_label = ticknode_shared::template::complete::type_label(ty, &Default::default());
        return Some(format!("{token_name}: {type_label}"));
    }

    if let Some(reference) = VariableRef::parse(token_name) {
        let ty = ed.reference_type(&reference)?;
        let title = ed.type_title(&ty);
        let label = ed
            .binding_label(&reference)
            .map(|(label, _)| label)
            .unwrap_or_else(|| token_name.into());
        return Some(format!("{label}: {title}"));
    }
    None
}
