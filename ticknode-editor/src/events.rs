//! Browser event adapters for the application-level editor actions.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::clipboard;
use crate::state::{Editor, now_ms};
use crate::{MIDDLE_PASTE_WINDOW_MS, execute_shortcut, shortcuts, typing_in_field};

pub fn keydown(editor: Editor, ev: web_sys::KeyboardEvent) {
    if editor.open_sheet.get_untracked().is_none() || typing_in_field(&ev) {
        return;
    }
    let ctrl = ev.ctrl_key() || ev.meta_key();
    if ctrl {
        // Keyboard paste is not a primary selection paste, even if a middle-button release was missed
        editor.middle_button_at.set_value(f64::NEG_INFINITY);
    }
    if !ctrl && editor.pending_paste.with_untracked(|p| p.is_none()) {
        if let Some(step) = crate::canvas::nudge_step(&ev.key(), ev.alt_key(), ev.shift_key()) {
            // Alt+Left/Right would otherwise navigate the browser history.
            if crate::canvas::nudge(editor, step) {
                ev.prevent_default();
            }
            return;
        }
    }
    let action = shortcuts::shortcut(
        ctrl,
        ev.shift_key(),
        &ev.key(),
        editor.pending_paste.with_untracked(|p| p.is_some()),
    );
    if let Some(action) = action {
        if action.prevents_default() {
            ev.prevent_default();
        }
        execute_shortcut(editor, action);
    }
}

pub fn hide_tooltip(editor: Editor, ev: web_sys::PointerEvent) {
    let on_info = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .and_then(|el| el.closest(".info-btn").ok().flatten())
        .is_some();
    if !on_info && editor.tooltip.with_untracked(|t| t.is_some()) {
        editor.tooltip.set(None);
    }
}

pub fn hide_picker(editor: Editor, ev: web_sys::PointerEvent) {
    let inside = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .and_then(|el| el.closest(".picker-popup, .picker-field").ok().flatten())
        .is_some();
    if !inside && editor.picker.with_untracked(|p| p.is_some()) {
        editor.picker.set(None);
    }
    if !inside && editor.template_completion.with_untracked(|p| p.is_some()) {
        editor.template_completion.set(None);
    }
}

pub fn paste(editor: Editor, ev: web_sys::ClipboardEvent) {
    if editor.open_sheet.get_untracked().is_none() || typing_in_field(&ev) {
        return;
    }
    if now_ms() - editor.middle_button_at.get_value() < MIDDLE_PASTE_WINDOW_MS {
        ev.prevent_default();
        return;
    }
    if let Some(text) = ev
        .clipboard_data()
        .and_then(|d| d.get_data("text/plain").ok())
    {
        ev.prevent_default();
        clipboard::paste_text(editor, &text);
    }
}
