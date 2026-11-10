//! Workspace toolbar.

use leptos::prelude::*;
use ticknode_shared::ClientMessage;
use crate::canvas::fit_view;
use crate::i18n::t;
use crate::logo::Logo;
use crate::route::{Route, navigate};
use crate::state::{ConnStatus, Theme, use_editor};
use crate::ui::{IconButton, StatusDot, icons};

#[component]
pub fn Header() -> impl IntoView {
    let ed = use_editor();
    let status = move || match ed.status.get() {
        ConnStatus::Online if ed.open_sheet.with(Option::is_none) => ("online", t!("status-online-home", count = ed.sheets.with(Vec::len))),
        ConnStatus::Online => ("online", t!("status-online", count = ed.nodes.with(|n| n.len()))),
        ConnStatus::Connecting => ("connecting", t!("status-connecting")),
        ConnStatus::Offline => ("offline", t!("status-offline")),
    };

    let is_sheet_page = move || ed.open_sheet.get().is_some();

    let current_sheet = move || {
        ed.open_sheet.get().and_then(|id| {
            ed.sheets.with(|list| list.iter().find(|s| s.info.id == id).cloned())
        })
    };

    let sheet_title = move || {
        current_sheet().map(|s| s.info.title).unwrap_or_else(|| "TickNode".to_string())
    };

    let is_sheet_disabled = move || {
        current_sheet().is_some_and(|s| !s.info.enabled)
    };

    view! {
        <header class="header">
            {move || is_sheet_page().then(|| view! {
                <button
                    class="icon-btn ctl header-back-btn"
                    title=move || t!("sheets-back")
                    aria-label=move || t!("sheets-back")
                    on:click=move |_| navigate(&Route::Home)
                >
                    <span class="mdi mdi-arrow-left" aria-hidden="true"></span>
                </button>
            })}
            <Logo />
            <div class="titles">
                <div class="header-title-row">
                    <span class="t-app-title ellipsis">
                        {move || if is_sheet_page() { sheet_title() } else { "TickNode".to_string() }}
                    </span>
                    {move || (is_sheet_page() && is_sheet_disabled()).then(|| view! {
                        <span class="chip st-idle">{move || t!("sheet-disabled")}</span>
                    })}
                </div>
                <span class="subtitle t-caption">
                    <StatusDot class=Signal::derive(move || status().0) />
                    <span class="ellipsis">{move || status().1}</span>
                </span>
            </div>
            <div class="actions">
                {move || is_sheet_page().then(|| view! {
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
                    <button
                        class="icon-btn ctl"
                        title=move || t!("toolbar-add-frame")
                        aria-label=move || t!("toolbar-add-frame")
                        on:click=move |_| crate::canvas::create_frame(ed)
                    >
                        <span class="mdi mdi-selection-drag" aria-hidden="true"></span>
                    </button>
                    <IconButton
                        icon=icons::TERMINAL
                        title=Signal::derive(move || t!("toolbar-console"))
                        selected=Signal::derive(move || ed.show_console.get())
                        on_click=Callback::new(move |_| ed.show_console.update(|v| *v = !*v))
                    />
                })}
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
