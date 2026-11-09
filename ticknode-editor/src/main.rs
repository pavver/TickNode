//! TickNode web editor built with Leptos (CSR). The engine runs on the server; the editor only
//! renders the graph and sends changes over WebSocket.

mod canvas;
mod categories;
mod clipboard;
mod connection;
mod geometry;
mod frozen;
mod events;
pub mod i18n;
mod inline;
mod match_editor;
mod panels;
mod picker;
mod node_menu;
mod quick_add;
mod plugins_panel;
mod shortcuts;
mod state;
mod ui;
mod toolbar;
mod wire_suggest;
mod wire_menu;
mod wire_context;

use std::time::Duration;

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::canvas::{Canvas, fit_view};
use crate::panels::{ConsolePanel, Inspector, Palette, PaletteGhost, TooltipLayer};
use crate::picker::PickerLayer;
use crate::plugins_panel::PluginsPanel;
use crate::state::{Editor, LogKind, Theme, now_ms};
use crate::toolbar::Header;
use ticknode_shared::ClientMessage;

/// Viewport width below which panels become slide-over overlays (see `@media` in `style.css`).
const NARROW_WIDTH: f64 = 900.0;
const THEME_KEY: &str = "ticknode.theme";
/// A `paste` event within this window after a middle-click is treated as a primary-selection paste.
const MIDDLE_PASTE_WINDOW_MS: f64 = 500.0;

fn window_width() -> f64 {
    window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0)
}

fn local_storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

fn load_theme() -> Theme {
    match local_storage().and_then(|s| s.get_item(THEME_KEY).ok().flatten()).as_deref() {
        Some("light") => Theme::Light,
        _ => Theme::Dark,
    }
}

/// Shortcuts and clipboard events are not intercepted while focus is inside an input control.
fn typing_in_field(ev: &web_sys::Event) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|el| {
            el.closest("input, textarea, select").ok().flatten().is_some()
                || el.dyn_ref::<web_sys::HtmlElement>().is_some_and(|el| el.is_content_editable())
        })
}

fn copy_selection(ed: Editor, ev: &web_sys::ClipboardEvent, cut: bool) {
    if typing_in_field(ev) {
        return;
    }
    let (Some(graph), Some(data)) = (clipboard::selection_snippet(ed), ev.clipboard_data()) else { return };
    let count = graph.nodes.len();
    if data.set_data("text/plain", &clipboard::to_text(graph)).is_ok() {
        ev.prevent_default();
        ed.log(
            LogKind::System,
            if cut {
                format!("[UI] Cut nodes: {count}")
            } else {
                format!("[UI] Copied nodes: {count}")
            },
        );
        if cut {
            clipboard::delete_selection(ed);
        }
    }
}

fn execute_shortcut(editor: Editor, action: shortcuts::Shortcut) {
    use shortcuts::Shortcut;
    match action {
        Shortcut::Delete => clipboard::delete_selection(editor),
        Shortcut::Escape => {
            editor.quick_add.set(None);
            editor.wire_menu.set(None);
            editor.wire_context.set(None);
            editor.selected_point.set(None);
            editor.copied_settings.set(None);
            if !clipboard::cancel_paste(editor) { editor.selection.set(None); }
        }
        Shortcut::Fit => fit_view(editor),
        Shortcut::Undo => editor.send(ClientMessage::Undo),
        Shortcut::Redo => editor.send(ClientMessage::Redo),
        Shortcut::SelectAll => editor.select_nodes(editor.nodes.with_untracked(|n| n.keys().copied().collect::<Vec<_>>())),
        Shortcut::Duplicate => clipboard::duplicate(editor),
    }
}

#[component]
fn App() -> impl IntoView {
    let locale = i18n::provide_locale(i18n::detect_locale());
    let editor = Editor::new(load_theme(), locale, window_width() > NARROW_WIDTH);
    editor.recent_nodes.set(quick_add::recent::load());
    provide_context(editor);

    Effect::new(move |_| {
        i18n::persist_locale(locale.get());
    });

    Effect::new(move |_| {
        let name = match editor.theme.get() {
            Theme::Dark => "dark",
            Theme::Light => "light",
        };
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("data-theme", name);
        }
        if let Some(storage) = local_storage() {
            let _ = storage.set_item(THEME_KEY, name);
        }
    });

    set_interval(move || editor.now.set(now_ms()), Duration::from_millis(100));

    let resize = window_event_listener(ev::resize, move |_| {
        editor.narrow.set(window_width() <= NARROW_WIDTH);
    });
    let keys = window_event_listener(ev::keydown, move |ev| events::keydown(editor, ev));
    // Copy/paste uses clipboard events: works without HTTPS and across browser tabs.
    let copy = window_event_listener(ev::copy, move |ev| copy_selection(editor, &ev, false));
    let cut = window_event_listener(ev::cut, move |ev| copy_selection(editor, &ev, true));
    // Middle button: record timestamp to ignore Linux primary-selection paste on middle-click pan.
    // Must use pointer events: the canvas cancels pointerdown, which suppresses mousedown/mouseup.
    let middle_down = window_event_listener(ev::pointerdown, move |ev| {
        if ev.button() == 1 {
            editor.middle_button_at.set_value(f64::INFINITY);
        }
    });
    let middle_up = window_event_listener(ev::pointerup, move |ev| {
        if ev.button() == 1 {
            editor.middle_button_at.set_value(now_ms());
        }
    });
    // The "info" tooltip is dismissed by pressing anywhere outside the info button itself
    let hide_tooltip = window_event_listener(ev::pointerdown, move |ev| events::hide_tooltip(editor, ev));
    // The property picker popup closes on a press anywhere outside it and outside its field
    let hide_picker = window_event_listener(ev::pointerdown, move |ev| events::hide_picker(editor, ev));
    let paste = window_event_listener(ev::paste, move |ev| events::paste(editor, ev));
    on_cleanup(move || {
        resize.remove();
        keys.remove();
        copy.remove();
        cut.remove();
        paste.remove();
        hide_tooltip.remove();
        hide_picker.remove();
        middle_down.remove();
        middle_up.remove();
    });

    connection::connect(editor);

    view! {
        <div class="app">
            <Header />
            <div class="workspace">
                <Palette />
                <div class="canvas-wrap"><Canvas /></div>
                <Inspector />
                <PluginsPanel />
            </div>
            <ConsolePanel />
            <PaletteGhost />
            <TooltipLayer />
            <PickerLayer />
            <wire_menu::WireMenuLayer />
            <quick_add::QuickAddLayer />
            <wire_context::WireContextLayer />
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
