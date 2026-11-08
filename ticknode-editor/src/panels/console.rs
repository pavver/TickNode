//! Editor console panel.

use leptos::prelude::*;
use crate::i18n::t;
use crate::state::use_editor;
use crate::ui::*;

// --- Console ---

#[component]
pub fn ConsolePanel() -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="console" class:hidden=move || !ed.show_console.get()>
            <div class="console-bar">
                <span class="t-section" style="flex:1">{move || t!("console-title")}</span>
                <span class="t-caption">{move || t!("console-lines", count = ed.logs.with(|l| l.len()))}</span>
                <IconButton icon=icons::BROOM title=Signal::derive(move || t!("console-clear")) on_click=Callback::new(move |_| ed.logs.set(Vec::new())) />
                <IconButton icon=icons::CLOSE title=Signal::derive(move || t!("console-hide")) on_click=Callback::new(move |_| ed.show_console.set(false)) />
            </div>
            <Terminal lines=ed.logs />
        </div>
    }
}
