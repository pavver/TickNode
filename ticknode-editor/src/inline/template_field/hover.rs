//! Hover token hit-testing and tooltip resolution for template fields.

use crate::i18n::t;
use crate::state::Editor;
use leptos::prelude::*;
use std::ops::Range;
use ticknode_shared::template::complete::expression_at;
use ticknode_shared::*;

/// Dotted identifier token chain inside a template expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverToken {
    pub range: Range<usize>,
    pub path: Vec<String>,
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Returns the dotted identifier chain at `byte` within `source`,
/// truncated to the segment under the pointer (e.g. hovering `b` in `a.b.c` yields `["a", "b"]`).
/// Returns `None` outside expressions, inside comments or string literals, or when not hovering an identifier.
pub fn token_at(source: &str, byte: usize) -> Option<HoverToken> {
    if byte >= source.len() {
        return None;
    }
    let bytes = source.as_bytes();
    if !is_ident_char(bytes[byte]) {
        return None;
    }
    if !expression_at(source, byte) {
        return None;
    }

    let mut seg_start = byte;
    while seg_start > 0 && is_ident_char(bytes[seg_start - 1]) {
        seg_start -= 1;
    }
    let mut seg_end = byte;
    while seg_end < bytes.len() && is_ident_char(bytes[seg_end]) {
        seg_end += 1;
    }

    let mut pos = seg_start;
    let mut preceding = Vec::new();
    loop {
        let mut p = pos;
        while p > 0 && bytes[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
        if p > 0 && bytes[p - 1] == b'.' {
            p -= 1;
            while p > 0 && bytes[p - 1].is_ascii_whitespace() {
                p -= 1;
            }
            let end_prev = p;
            while p > 0 && is_ident_char(bytes[p - 1]) {
                p -= 1;
            }
            if p < end_prev {
                preceding.push(source[p..end_prev].to_string());
                pos = p;
                continue;
            }
        }
        break;
    }
    preceding.reverse();
    preceding.push(source[seg_start..seg_end].to_string());

    Some(HoverToken {
        range: seg_start..seg_end,
        path: preceding,
    })
}

/// Returns the ranges of all identifier tokens inside template expressions in `source`.
pub fn find_expression_tokens(source: &str) -> Vec<Range<usize>> {
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
            let end = i;
            if expression_at(source, start) {
                tokens.push(start..end);
            }
        } else if let Some(ch) = source[i..].chars().next() {
            i += ch.len_utf8();
        } else {
            i += 1;
        }
    }
    tokens
}

/// Converts pointer coordinates into the hovered template identifier token.
#[cfg(target_arch = "wasm32")]
pub fn hit_test_token(
    textarea: &web_sys::HtmlTextAreaElement,
    client_x: f64,
    client_y: f64,
    source: &str,
) -> Option<HoverToken> {
    let ta_rect = textarea.get_bounding_client_rect();
    if client_x < ta_rect.left()
        || client_x > ta_rect.right()
        || client_y < ta_rect.top()
        || client_y > ta_rect.bottom()
    {
        return None;
    }

    let tokens = find_expression_tokens(source);
    if tokens.is_empty() {
        return None;
    }

    let mirror = super::caret::Mirror::new(textarea)?;
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
            spans.push((token_range.start, span));
        }
        last_end = token_range.end;
    }
    if last_end < source.len() {
        let _ = mirror
            .div
            .append_child(&mirror.document.create_text_node(&source[last_end..]));
    }
    let frame = mirror.frame(textarea);
    let hit = spans.iter().find_map(|(start, span)| {
        let (x, y, w, h) = frame.screen_rect(&span.get_bounding_client_rect());
        (client_x >= x && client_x <= x + w && client_y >= y && client_y <= y + h).then_some(*start)
    });
    mirror.remove();
    hit.and_then(|start| token_at(source, start))
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn hit_test_token(
    _textarea: &web_sys::HtmlTextAreaElement,
    _client_x: f64,
    _client_y: f64,
    _source: &str,
) -> Option<HoverToken> {
    None
}

/// Resolves the nested struct field type from `root_type` along `fields`.
pub fn resolve_field_path_type(
    root_type: &DataType,
    fields: &[String],
    types: &TypeRegistry,
) -> Option<DataType> {
    let mut current = root_type.clone();
    for field in fields {
        match &current {
            DataType::Named(name) => {
                let def = types.get(name)?;
                match &def.kind {
                    TypeKind::Struct {
                        fields: struct_fields,
                    } => {
                        let f = struct_fields.iter().find(|sf| sf.name == *field)?;
                        current = f.data_type.clone();
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Descends into `DataValue::Object` along `fields`.
pub fn resolve_field_path_value(
    mut current: Option<&DataValue>,
    fields: &[String],
) -> Option<DataValue> {
    for field in fields {
        match current {
            Some(DataValue::Object {
                fields: obj_fields, ..
            }) => {
                current = obj_fields.get(field).map(|f| &f.value);
            }
            _ => return None,
        }
    }
    current.cloned()
}

/// Formats the field hover tooltip (type line and value line).
pub fn format_field_tooltip(
    path: &[String],
    field_type: &DataType,
    value: Option<&DataValue>,
    type_title: &str,
) -> String {
    let path_str = path.join(".");
    let line1 = format!("{path_str}: {type_title}");
    let line2 = match value {
        Some(v) => crate::node_status::truncate(crate::i18n::value_as_text(v, field_type), 200),
        None => t!("pin-no-value"),
    };
    format!("{line1}\n{line2}")
}

/// Resolves the tooltip text for a hovered path in the template of `node`.
#[allow(dead_code)]
pub fn resolve_hover_tooltip(ed: Editor, node: NodeId, path: &[String]) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let client_node = ed.node_tracked(node)?;
    let root = &path[0];

    // Case 2: root is `text` and second segment is an input name
    if root == "text" && path.len() == 2 {
        let input_name = &path[1];
        let pin = client_node.instance.with(|i| {
            i.definition
                .pins
                .iter()
                .find(|p| p.direction == PinDirection::Input && p.name == *input_name)
                .cloned()
        })?;
        let pin_title = crate::canvas::pin_title_text(ed, node, &pin, PinDirection::Input);
        let header = t!("template-hover-text-of", name = &pin.name);
        return Some(format!("{header}\n{pin_title}"));
    }

    let pin = client_node.instance.with(|i| {
        i.definition
            .pins
            .iter()
            .find(|p| p.direction == PinDirection::Input && p.name == *root)
            .cloned()
    })?;

    // Case 1: root is an input name and the path has one segment
    if path.len() == 1 {
        return Some(crate::canvas::pin_title_text(
            ed,
            node,
            &pin,
            PinDirection::Input,
        ));
    }

    // Case 3: root is an input and the path has fields
    let declared_ty = match &pin.pin_type {
        PinType::Data(t) => t.clone(),
        _ => DataType::Any,
    };
    let eff_type = crate::canvas::display_type(&declared_ty, ed.source_type(node, pin.id));
    let fields = &path[1..];
    let field_type = ed
        .types
        .with(|types| resolve_field_path_type(&eff_type, fields, types))?;

    let preview_inputs = crate::inline::template_field::preview::node_preview_inputs(ed, node);
    let input_val = preview_inputs
        .into_iter()
        .find(|(name, _, _)| name == root)
        .and_then(|(_, v, _)| v);
    let field_val = resolve_field_path_value(input_val.as_ref(), fields);
    let type_title = ed.type_title(&field_type);
    Some(format_field_tooltip(
        path,
        &field_type,
        field_val.as_ref(),
        &type_title,
    ))
}
