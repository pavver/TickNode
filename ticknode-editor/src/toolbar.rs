//! Workspace toolbar.

mod title;

use crate::canvas::fit_view;
use crate::i18n::t;
use crate::logo::Logo;
use crate::route::{Route, navigate};
use crate::state::{ConnStatus, Theme, use_editor};
use crate::ui::{StatusDot, op_icons};
use leptos::prelude::*;
use operator_ui::prelude::{ControlState, IconButton, icons};
use ticknode_shared::ClientMessage;

#[component]
pub fn Header() -> impl IntoView {
    let ed = use_editor();
    let status = move || match ed.status.get() {
        ConnStatus::Online if ed.open_sheet.with(Option::is_none) => (
            "online",
            t!("status-online-home", count = ed.sheets.with(Vec::len)),
        ),
        ConnStatus::Online => (
            "online",
            t!("status-online", count = ed.nodes.with(|n| n.len())),
        ),
        ConnStatus::Connecting => ("connecting", t!("status-connecting")),
        ConnStatus::Offline => ("offline", t!("status-offline")),
    };

    let is_sheet_page = move || ed.open_sheet.get().is_some();

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
            {move || if is_sheet_page() {
                view! {
                    <title::SheetTitle
                        status_class=Signal::derive(move || status().0)
                        status_text=Signal::derive(move || status().1)
                    />
                }.into_any()
            } else {
                view! {
                    <div class="titles">
                        <div class="header-title-row">
                            <span class="t-app-title ellipsis">"TickNode"</span>
                        </div>
                        <span class="subtitle t-caption">
                            <StatusDot class=Signal::derive(move || status().0) />
                            <span class="ellipsis">{move || status().1}</span>
                        </span>
                    </div>
                }.into_any()
            }}
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
                    {move || view! {
                        <IconButton
                            size=9.0
                            icon=icons::ADD
                            label=t!("toolbar-palette")
                            state=Signal::derive(move || ed.show_palette.get().then_some(ControlState::Selected))
                            on_click=Callback::new(move |_| ed.show_palette.update(|v| *v = !*v))
                        />
                    }}
                    <button class="icon-btn ctl" class:st-selected=move || ed.show_values.get()
                        title=move || t!("toolbar-show-values") aria-label=move || t!("toolbar-show-values")
                        aria-pressed=move || ed.show_values.get().to_string()
                        on:click=move |_| crate::node_status::toggle_show_values(ed)>
                        <span class=move || if ed.show_values.get() { "mdi mdi-eye-outline" } else { "mdi mdi-eye-off-outline" } aria-hidden="true"></span>
                    </button>
                    {move || view! {
                        <IconButton
                            size=9.0
                            icon=op_icons::FIT
                            label=t!("toolbar-fit")
                            on_click=Callback::new(move |_| fit_view(ed))
                        />
                    }}
                    <button
                        class="icon-btn ctl"
                        title=move || t!("toolbar-add-frame")
                        aria-label=move || t!("toolbar-add-frame")
                        on:click=move |_| crate::canvas::create_frame(ed)
                    >
                        <span class="mdi mdi-selection-drag" aria-hidden="true"></span>
                    </button>
                    <ControlsHelp />
                    {move || view! {
                        <IconButton
                            size=9.0
                            icon=op_icons::TERMINAL
                            label=t!("toolbar-console")
                            state=Signal::derive(move || ed.show_console.get().then_some(ControlState::Selected))
                            on_click=Callback::new(move |_| ed.show_console.update(|v| *v = !*v))
                        />
                    }}
                })}
                {move || view! {
                    <IconButton
                        size=9.0
                        icon=op_icons::THEME
                        label=t!("toolbar-theme")
                        on_click=Callback::new(move |_| ed.theme.update(|t| *t = if *t == Theme::Dark { Theme::Light } else { Theme::Dark }))
                    />
                }}
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

/// Hint keys shown in the canvas controls help, in order.
const CONTROL_HINTS: [&str; 13] = [
    "overview-hint-1",
    "overview-hint-2",
    "overview-hint-3",
    "overview-hint-4",
    "overview-hint-5",
    "overview-hint-6",
    "overview-hint-7",
    "overview-hint-8",
    "overview-hint-9",
    "overview-hint-resize",
    "overview-hint-quick-add",
    "overview-hint-frame",
    "overview-hint-nudge",
];

/// Toolbar help button: graph statistics and the canvas controls in a hover card.
#[component]
fn ControlsHelp() -> impl IntoView {
    let ed = use_editor();
    let stats = move || {
        let nodes = ed.nodes.with(|n| n.len());
        let conns = ed.exec.with(|e| e.len()) + ed.data.with(|d| d.len());
        t!("overview-stats", nodes = nodes, connections = conns)
    };
    let card = move || {
        view! {
            <div class="node-help">
                <div class="op-t-row-title">{move || t!("overview-title-controls")}</div>
                <div class="op-t-caption node-help-meta">{stats}</div>
                {CONTROL_HINTS.iter().map(|key| view! { <p class="node-help-text">{move || t!(key)}</p> }).collect_view()}
            </div>
        }
    };
    view! {
        <operator_ui::prelude::HoverCard card=ViewFn::from(card) width=110.0 label=t!("toolbar-help")>
            <button class="icon-btn ctl" aria-label=move || t!("toolbar-help")>
                <span class="mdi mdi-help-circle-outline" aria-hidden="true"></span>
            </button>
        </operator_ui::prelude::HoverCard>
    }
}
