//! Workspace toolbar.

use leptos::prelude::*;
use ticknode_shared::ClientMessage;
use crate::canvas::fit_view;
use crate::i18n::t;
use crate::state::{ConnStatus, Theme, use_editor};
use crate::ui::{IconButton, StatusDot, icons};

#[component]
pub fn Header() -> impl IntoView {
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
                <button
                    class="icon-btn ctl"
                    class:st-disabled=move || !ed.history.get().can_undo
                    title=move || t!("toolbar-undo")
                    aria-label=move || t!("toolbar-undo")
                    disabled=move || !ed.history.get().can_undo
                    on:click=move |_| ed.send(ClientMessage::Undo)
                >
                    <span class="mdi mdi-undo" aria-hidden="true"></span>
                </button>
                <button
                    class="icon-btn ctl"
                    class:st-disabled=move || !ed.history.get().can_redo
                    title=move || t!("toolbar-redo")
                    aria-label=move || t!("toolbar-redo")
                    disabled=move || !ed.history.get().can_redo
                    on:click=move |_| ed.send(ClientMessage::Redo)
                >
                    <span class="mdi mdi-redo" aria-hidden="true"></span>
                </button>
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
                <button class="icon-btn ctl" class:st-selected=move || ed.show_values.get()
                    title=move || t!("toolbar-show-values") aria-label=move || t!("toolbar-show-values")
                    aria-pressed=move || ed.show_values.get().to_string()
                    on:click=move |_| crate::node_status::toggle_show_values(ed)>
                    <span class=move || if ed.show_values.get() { "mdi mdi-eye-outline" } else { "mdi mdi-eye-off-outline" } aria-hidden="true"></span>
                </button>
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

