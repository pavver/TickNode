//! Editor console panel.

use crate::i18n::t;
use crate::state::use_editor;
use crate::ui::{log_text, op_icons};
use leptos::prelude::*;
use operator_ui::prelude::{ConsoleTerminal, IconButton, icons};

// --- Console ---

#[component]
pub fn ConsolePanel() -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="console" class:hidden=move || !ed.show_console.get()>
            <div class="console-bar">
                <span class="t-section" style="flex:1">{move || t!("console-title")}</span>
                <span class="t-caption">{move || t!("console-lines", count = ed.logs.with(|l| l.len()))}</span>
                {move || view! {
                    <IconButton icon=op_icons::BROOM label=t!("console-clear") size=7.0 on_click=Callback::new(move |_| ed.logs.set(Vec::new())) />
                }}
                {move || view! {
                    <IconButton icon=icons::CLOSE label=t!("console-hide") size=7.0 on_click=Callback::new(move |_| ed.show_console.set(false)) />
                }}
            </div>
            <ConsoleTerminal logs=Signal::derive(move || ed.logs.with(|logs| logs.iter().map(log_text).collect::<Vec<_>>())) />
        </div>
    }
}
