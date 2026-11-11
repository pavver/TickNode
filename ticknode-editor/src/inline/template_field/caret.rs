//! Caret UTF-16 / UTF-8 conversion, completion splicing, and DOM mirror measurement.

use std::ops::Range;

/// Converts a UTF-16 code unit offset into a UTF-8 byte offset in `text`.
/// Clamps to `text.len()`. If the offset lands inside a surrogate pair, aligns to char boundary.
pub fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut current_u16 = 0;
    for (byte_offset, ch) in text.char_indices() {
        if current_u16 >= utf16_offset {
            return byte_offset;
        }
        let next_u16 = current_u16 + ch.len_utf16();
        if next_u16 > utf16_offset {
            return byte_offset;
        }
        current_u16 = next_u16;
    }
    text.len()
}

/// Converts a UTF-8 byte offset into a UTF-16 code unit offset in `text`.
/// Clamps to total UTF-16 length. If `byte_offset` lands within a multibyte char, aligns to char boundary.
pub fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    let mut current_u16 = 0;
    for (b_offset, ch) in text.char_indices() {
        if b_offset >= byte_offset {
            return current_u16;
        }
        if b_offset + ch.len_utf8() > byte_offset {
            return current_u16;
        }
        current_u16 += ch.len_utf16();
    }
    current_u16
}

/// Replaces `range` with `insert` in `source`, returning the updated string and caret byte offset.
/// If `insert` ends with `()` (e.g. functions), the caret is placed between the parentheses.
pub fn apply_completion(source: &str, range: Range<usize>, insert: &str) -> (String, usize) {
    let start = range.start.min(source.len());
    let end = range.end.min(source.len()).max(start);
    let mut result = String::with_capacity(source.len() + insert.len() - (end - start));
    result.push_str(&source[..start]);
    result.push_str(insert);
    result.push_str(&source[end..]);

    let caret_byte = if insert.ends_with("()") {
        start + insert.len() - 1
    } else {
        start + insert.len()
    };
    (result, caret_byte)
}

/// Computes pixel coordinates of the caret in a `<textarea>` using the mirror-div technique.
/// Returns `(caret_x, caret_bottom_y, caret_top_y)`.
#[cfg(target_arch = "wasm32")]
pub fn measure_caret_screen_pos(
    textarea: &web_sys::HtmlTextAreaElement,
    caret_byte: usize,
    source: &str,
) -> Option<(f64, f64, f64)> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window()?;
    let document = window.document()?;
    let computed = window.get_computed_style(textarea).ok().flatten()?;

    let div = document.create_element("div").ok()?;
    let div_style = div.dyn_ref::<web_sys::HtmlElement>()?.style();

    // Mirror textarea geometry and typography
    let _ = div_style.set_property("position", "absolute");
    let _ = div_style.set_property("visibility", "hidden");
    let _ = div_style.set_property("top", "0");
    let _ = div_style.set_property("left", "0");
    let _ = div_style.set_property("box-sizing", &computed.get_property_value("box-sizing").unwrap_or_default());
    let _ = div_style.set_property("width", &computed.get_property_value("width").unwrap_or_default());
    let _ = div_style.set_property("font-family", &computed.get_property_value("font-family").unwrap_or_default());
    let _ = div_style.set_property("font-size", &computed.get_property_value("font-size").unwrap_or_default());
    let _ = div_style.set_property("font-weight", &computed.get_property_value("font-weight").unwrap_or_default());
    let _ = div_style.set_property("font-style", &computed.get_property_value("font-style").unwrap_or_default());
    let _ = div_style.set_property("letter-spacing", &computed.get_property_value("letter-spacing").unwrap_or_default());
    let _ = div_style.set_property("line-height", &computed.get_property_value("line-height").unwrap_or_default());
    let _ = div_style.set_property("padding-top", &computed.get_property_value("padding-top").unwrap_or_default());
    let _ = div_style.set_property("padding-right", &computed.get_property_value("padding-right").unwrap_or_default());
    let _ = div_style.set_property("padding-bottom", &computed.get_property_value("padding-bottom").unwrap_or_default());
    let _ = div_style.set_property("padding-left", &computed.get_property_value("padding-left").unwrap_or_default());
    let _ = div_style.set_property("border-top-width", &computed.get_property_value("border-top-width").unwrap_or_default());
    let _ = div_style.set_property("border-right-width", &computed.get_property_value("border-right-width").unwrap_or_default());
    let _ = div_style.set_property("border-bottom-width", &computed.get_property_value("border-bottom-width").unwrap_or_default());
    let _ = div_style.set_property("border-left-width", &computed.get_property_value("border-left-width").unwrap_or_default());
    let _ = div_style.set_property("white-space", "pre-wrap");
    let _ = div_style.set_property("word-wrap", "break-word");
    let _ = div_style.set_property("overflow-wrap", "break-word");
    let _ = div_style.set_property("overflow", "hidden");

    let text_before = if caret_byte <= source.len() { &source[..caret_byte] } else { source };
    let text_node = document.create_text_node(text_before);
    let _ = div.append_child(&text_node);

    let marker = document.create_element("span").ok()?;
    marker.set_text_content(Some("\u{200b}"));
    let _ = div.append_child(&marker);

    let body = document.body()?;
    let _ = body.append_child(&div);

    let marker_rect = marker.get_bounding_client_rect();
    let div_rect = div.get_bounding_client_rect();

    let span_x = marker_rect.left() - div_rect.left();
    let span_y = marker_rect.top() - div_rect.top();
    let span_h = marker_rect.height().max(16.0);

    let _ = body.remove_child(&div);

    let ta_rect = textarea.get_bounding_client_rect();
    let scroll_top = textarea.scroll_top() as f64;
    let scroll_left = textarea.scroll_left() as f64;

    let border_left = computed.get_property_value("border-left-width")
        .ok()
        .and_then(|v| v.strip_suffix("px").and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0);
    let border_top = computed.get_property_value("border-top-width")
        .ok()
        .and_then(|v| v.strip_suffix("px").and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0);

    let caret_x = ta_rect.left() + border_left + span_x - scroll_left;
    let caret_top = ta_rect.top() + border_top + span_y - scroll_top;
    let caret_bottom = caret_top + span_h;

    Some((caret_x, caret_bottom, caret_top))
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn measure_caret_screen_pos(
    _textarea: &web_sys::HtmlTextAreaElement,
    _caret_byte: usize,
    _source: &str,
) -> Option<(f64, f64, f64)> {
    None
}
