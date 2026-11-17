//! Sheet title in the header: text with a pencil button that swaps it for a text field.

use leptos::prelude::*;
use operator_ui::prelude::{ActionResult, AsyncAction, IconButton, TextField, icons};
use ticknode_shared::{ClientMessage, SheetInfo};
use wasm_bindgen::JsCast;

use crate::i18n::t;
use crate::state::use_editor;

/// Title of the open sheet with its status line; the pencil renames the sheet in place.
#[component]
pub(super) fn SheetTitle(
    #[prop(into)] status_class: Signal<&'static str>,
    #[prop(into)] status_text: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let editing = RwSignal::new(false);
    let draft = RwSignal::new(String::new());
    let field = NodeRef::<leptos::html::Div>::new();
    let current = move || {
        ed.open_sheet.get().and_then(|id| {
            ed.sheets
                .with(|list| list.iter().find(|s| s.info.id == id).map(|s| s.info.clone()))
        })
    };
    let start = Callback::new(move |()| {
        let Some(info) = current() else { return };
        draft.set(info.title);
        editing.set(true);
        // Focus the field once it is rendered.
        request_animation_frame(move || {
            if let Some(input) = field
                .get_untracked()
                .and_then(|el| el.query_selector("input").ok().flatten())
                .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                let _ = input.focus();
                input.select();
            }
        });
    });
    let save = AsyncAction::new(move |title: String| {
        let title = title.trim().to_string();
        let result = match current() {
            _ if title.is_empty() => ActionResult::Error(Some(t!("sheet-title-empty"))),
            Some(info) => {
                if title != info.title {
                    ed.send(ClientMessage::UpdateSheet(SheetInfo { title, ..info }));
                }
                editing.set(false);
                ActionResult::Success
            }
            None => {
                editing.set(false);
                ActionResult::Success
            }
        };
        async move { result }
    });
    let disabled = move || current().is_some_and(|s| !s.enabled);
    view! {
        <Show
            when=move || editing.get()
            fallback=move || view! {
                <div class="titles">
                    <div class="header-title-row">
                        <span class="t-app-title ellipsis">{move || current().map(|s| s.title).unwrap_or_default()}</span>
                        <IconButton icon=icons::EDIT label=t!("sheet-rename") on_click=start />
                        {move || disabled().then(|| view! {
                            <span class="chip st-idle">{move || t!("sheet-disabled")}</span>
                        })}
                    </div>
                    <span class="subtitle t-caption">
                        <crate::ui::StatusDot class=status_class />
                        <span class="ellipsis">{move || status_text.get()}</span>
                    </span>
                </div>
            }
        >
            <div
                class="titles sheet-title-edit"
                node_ref=field
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        ev.prevent_default();
                        ev.stop_propagation();
                        editing.set(false);
                    }
                }
            >
                <TextField
                    value=draft
                    placeholder=t!("sheet-title-label")
                    label=t!("sheet-title-label")
                    save_text=t!("sheet-save")
                    save_icon=icons::CHECK
                    on_save=save.clone()
                />
            </div>
            <IconButton icon=icons::CLOSE label=t!("sheet-cancel") on_click=Callback::new(move |()| editing.set(false)) />
        </Show>
    }
}
