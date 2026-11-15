//! Undoable programmatic text editing for template and formula textareas.
//!
//! Uses the browser's native `document.execCommand` ("insertText" / "delete") so that
//! auto-closed brackets and accepted completions preserve the browser's undo stack (Ctrl+Z).
//! Falls back to `HtmlTextAreaElement::set_value` if `execCommand` is unavailable or fails.

use std::ops::Range;

#[cfg(target_arch = "wasm32")]
use crate::inline::template_field::caret::utf16_to_byte;
use crate::inline::template_field::caret::{apply_completion, byte_to_utf16};

#[cfg(test)]
mod tests;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
/// Converts a byte range in `text` to a UTF-16 code unit offset range `(start_u16, end_u16)`.
pub fn utf16_range(text: &str, range: Range<usize>) -> (u32, u32) {
    let start_u16 = byte_to_utf16(text, range.start) as u32;
    let end_u16 = byte_to_utf16(text, range.end) as u32;
    (start_u16, end_u16)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
/// Pure calculation for an edit: given current text, byte range to replace, replacement text,
/// and the desired caret byte offset in the resulting text, computes:
/// `(start_utf16, end_utf16, caret_utf16, new_text)`.
pub fn plan_replace(
    current: &str,
    range: Range<usize>,
    insert: &str,
    new_caret_byte: usize,
) -> (u32, u32, u32, String) {
    let (new_text, _) = apply_completion(current, range.clone(), insert, None);
    let (start_u16, end_u16) = utf16_range(current, range);
    let caret_u16 = byte_to_utf16(&new_text, new_caret_byte) as u32;
    (start_u16, end_u16, caret_u16, new_text)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
/// Pure calculation for accepting a completion item: given current text, byte range to replace,
/// replacement text, and optional caret offset within the inserted snippet, computes:
/// `(start_utf16, end_utf16, caret_utf16, new_text, new_caret_byte)`.
pub fn plan_completion(
    current: &str,
    range: Range<usize>,
    insert: &str,
    caret: Option<usize>,
) -> (u32, u32, u32, String, usize) {
    let (new_text, new_caret_byte) = apply_completion(current, range.clone(), insert, caret);
    let (start_u16, end_u16) = utf16_range(current, range);
    let caret_u16 = byte_to_utf16(&new_text, new_caret_byte) as u32;
    (start_u16, end_u16, caret_u16, new_text, new_caret_byte)
}

/// Programmatically replaces `start_utf16..end_utf16` in `textarea` with `text` while preserving
/// the browser's native undo history.
///
/// If `exec_command` succeeds, the browser automatically dispatches a native `input` event,
/// so no synthetic event is fired.
/// If `exec_command` fails or returns false, falls back to `textarea.set_value` and dispatches
/// a synthetic `input` event so reactive listeners update.
/// Finally sets the selection / caret to `caret_utf16` and returns the new `textarea.value()`.
#[cfg(target_arch = "wasm32")]
pub fn replace_range(
    textarea: &web_sys::HtmlTextAreaElement,
    start_utf16: u32,
    end_utf16: u32,
    text: &str,
    caret_utf16: u32,
) -> String {
    use wasm_bindgen::JsCast;

    let _ = textarea.focus();
    let _ = textarea.set_selection_range(start_utf16, end_utf16);

    let mut applied = false;
    if let Some(doc) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok())
    {
        let res = if text.is_empty() {
            if start_utf16 == end_utf16 {
                Ok(true)
            } else {
                doc.exec_command("delete")
            }
        } else {
            doc.exec_command_with_show_ui_and_value("insertText", false, text)
        };
        if res.unwrap_or(false) {
            applied = true;
        }
    }

    if !applied {
        let current = textarea.value();
        let start_byte = utf16_to_byte(&current, start_utf16 as usize);
        let end_byte = utf16_to_byte(&current, end_utf16 as usize);
        let mut fallback =
            String::with_capacity(current.len() + text.len() - end_byte.saturating_sub(start_byte));
        fallback.push_str(&current[..start_byte]);
        fallback.push_str(text);
        fallback.push_str(&current[end_byte..]);
        textarea.set_value(&fallback);
        if let Ok(ev) = web_sys::Event::new("input") {
            let _ = textarea.dispatch_event(&ev);
        }
    }

    let _ = textarea.set_selection_range(caret_utf16, caret_utf16);
    textarea.value()
}
