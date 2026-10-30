//! TickNode web editor built with Leptos (CSR). The engine runs on the server; the editor only
//! renders the graph and sends changes over WebSocket.

mod canvas;
mod categories;
mod clipboard;
mod connection;
mod geometry;
pub mod i18n;
mod inline;
mod match_editor;
mod panels;
mod plugins_panel;
mod state;
mod ui;

use std::time::Duration;

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::canvas::{Canvas, fit_view};
use crate::i18n::t;
use crate::panels::{ConsolePanel, Inspector, Palette, PaletteGhost, TooltipLayer};
use crate::plugins_panel::PluginsPanel;
use crate::state::{ConnStatus, Editor, LogKind, Theme, now_ms, use_editor};
use crate::ui::{IconButton, StatusDot, icons};

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
        .is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
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
                t!("log-ui-cut", count = count)
            } else {
                t!("log-ui-copied", count = count)
            },
        );
        if cut {
            clipboard::delete_selection(ed);
        }
    }
}

#[component]
fn App() -> impl IntoView {
    let locale = i18n::provide_locale(i18n::detect_locale());
    let editor = Editor::new(load_theme(), locale, window_width() > NARROW_WIDTH);
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
    let keys = window_event_listener(ev::keydown, move |ev| {
        if typing_in_field(&ev) {
            return;
        }
        let ctrl = ev.ctrl_key() || ev.meta_key();
        if ctrl {
            // Keyboard paste is not a primary selection paste, even if a middle-button release was missed
            editor.middle_button_at.set_value(f64::NEG_INFINITY);
        }
        match (ctrl, ev.key().to_lowercase().as_str()) {
            (false, "delete" | "backspace") => clipboard::delete_selection(editor),
            (false, "escape") => {
                if !clipboard::cancel_paste(editor) {
                    editor.selection.set(None);
                }
            }
            (false, "f" | "а") => fit_view(editor),
            (true, "a" | "ф") => {
                ev.prevent_default();
                editor.select_nodes(editor.nodes.with_untracked(|n| n.keys().copied().collect::<Vec<_>>()));
            }
            (true, "d" | "в") => {
                ev.prevent_default();
                clipboard::duplicate(editor);
            }
            _ => {}
        }
    });
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
    let hide_tooltip = window_event_listener(ev::pointerdown, move |ev| {
        let on_info = ev.target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".info-btn").ok().flatten())
            .is_some();
        if !on_info && editor.tooltip.with_untracked(|t| t.is_some()) {
            editor.tooltip.set(None);
        }
    });
    let paste = window_event_listener(ev::paste, move |ev| {
        if typing_in_field(&ev) {
            return;
        }
        if now_ms() - editor.middle_button_at.get_value() < MIDDLE_PASTE_WINDOW_MS {
            ev.prevent_default();
            return;
        }
        if let Some(text) = ev.clipboard_data().and_then(|d| d.get_data("text/plain").ok()) {
            ev.prevent_default();
            clipboard::paste_text(editor, &text);
        }
    });
    on_cleanup(move || {
        resize.remove();
        keys.remove();
        copy.remove();
        cut.remove();
        paste.remove();
        hide_tooltip.remove();
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
        </div>
    }
}

#[component]
fn Header() -> impl IntoView {
    let ed = use_editor();
    let status = move || match ed.status.get() {
        ConnStatus::Online => ("online", t!("status-online", count = ed.nodes.with(|n| n.len()))),
        ConnStatus::Connecting => ("connecting", t!("status-connecting")),
        ConnStatus::Offline => ("offline", t!("status-offline")),
    };
    view! {
        <header class="header">
            <div class="titles">
                <span class="t-app-title ellipsis">"TickNode"</span>
                <span class="subtitle t-caption">
                    <StatusDot class=Signal::derive(move || status().0) />
                    <span class="ellipsis">{move || status().1}</span>
                </span>
            </div>
            <div class="actions">
                <IconButton
                    icon=icons::ADD
                    title=Signal::derive(move || t!("toolbar-palette"))
                    selected=Signal::derive(move || ed.show_palette.get())
                    on_click=Callback::new(move |_| ed.show_palette.update(|v| *v = !*v))
                />
                <IconButton
                    icon=icons::EXTENSION
                    title=Signal::derive(move || t!("toolbar-plugins"))
                    selected=Signal::derive(move || ed.show_plugins.get())
                    on_click=Callback::new(move |_| ed.show_plugins.update(|v| *v = !*v))
                />
                <IconButton
                    icon=icons::FIT
                    title=Signal::derive(move || t!("toolbar-fit"))
                    on_click=Callback::new(move |_| fit_view(ed))
                />
                <IconButton
                    icon=icons::TERMINAL
                    title=Signal::derive(move || t!("toolbar-console"))
                    selected=Signal::derive(move || ed.show_console.get())
                    on_click=Callback::new(move |_| ed.show_console.update(|v| *v = !*v))
                />
                <IconButton
                    icon=icons::THEME
                    title=Signal::derive(move || t!("toolbar-theme"))
                    on_click=Callback::new(move |_| ed.theme.update(|t| *t = if *t == Theme::Dark { Theme::Light } else { Theme::Dark }))
                />
                <button
                    class="icon-btn ctl lang-btn"
                    title=move || t!("toolbar-language")
                    aria-label=move || t!("toolbar-language")
                    on:click=move |_| ed.locale.update(|l| *l = l.toggle())
                >
                    {move || ed.locale.get().badge()}
                </button>
            </div>
        </header>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
