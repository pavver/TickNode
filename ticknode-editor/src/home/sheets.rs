//! Scenario sheets section for the home page.

use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;

use crate::home::logic::{can_move_down, can_move_up, sheet_last_run_age};
use crate::i18n::t;
use crate::panels::relative_time;
use crate::route::{Route, navigate};
use crate::state::use_editor;
use crate::ui::Switch;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardMode {
    Normal,
    Editing,
    Deleting,
}

fn is_control(ev: &web_sys::MouseEvent) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|el| {
            el.closest("button, input, textarea, select, a, .switch")
                .ok()
                .flatten()
                .is_some()
        })
}

#[component]
pub fn SheetsSection() -> impl IntoView {
    let ed = use_editor();

    let on_new_sheet = move |_| {
        ed.pending_create_tracker.update(|t| t.request_create());
        ed.send(ClientMessage::CreateSheet {
            title: t!("sheet-new-title"),
            caption: String::new(),
        });
    };

    // Only the empty/non-empty switch may rebuild the list: rebuilding on every `Sheets`
    // update would recreate the cards and reset an open edit form or delete confirmation.
    let empty = Memo::new(move |_| ed.sheets.with(Vec::is_empty));

    view! {
        <div class="home-sheets-section">
            <div class="home-section-toolbar">
                <button class="btn primary ctl" on:click=on_new_sheet>
                    <span class="mdi mdi-plus" aria-hidden="true"></span>
                    <span>{move || t!("sheet-new")}</span>
                </button>
            </div>
            {move || if empty.get() {
                view! { <div class="empty sheets-empty">{move || t!("sheets-empty")}</div> }.into_any()
            } else {
                view! {
                    <div class="sheet-cards">
                        <For each=move || ed.sheets.get() key=|s| s.info.id let:summary>
                            <SheetCard summary=summary />
                        </For>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn SheetCard(summary: SheetSummary) -> impl IntoView {
    let ed = use_editor();
    let id = summary.info.id;
    // The card lives as long as its id; its contents follow the latest `Sheets` summary.
    let summary = Memo::new(move |_| {
        ed.sheets
            .with(|list| list.iter().find(|s| s.info.id == id).cloned())
            .unwrap_or_else(|| summary.clone())
    });
    let mode = RwSignal::new(CardMode::Normal);
    let draft_title = RwSignal::new(summary.with_untracked(|s| s.info.title.clone()));
    let draft_caption = RwSignal::new(summary.with_untracked(|s| s.info.caption.clone()));

    let index = move || {
        ed.sheets
            .with(|list| list.iter().position(|s| s.info.id == id).unwrap_or(0))
    };
    let total = move || ed.sheets.with(|list| list.len());
    let can_up = move || can_move_up(index());
    let can_down = move || can_move_down(index(), total());

    let last_run = move || {
        let last_exec = summary.with(|s| s.last_executed_ms);
        let age = sheet_last_run_age(last_exec, ed.sheets_received_at.get(), ed.now.get());
        match age {
            Some(ms) => t!("sheet-last-run", time = relative_time(ms)),
            None => t!("sheet-never-run"),
        }
    };

    let on_card_click = move |ev: web_sys::MouseEvent| {
        if mode.get() == CardMode::Normal && !is_control(&ev) {
            navigate(&Route::Sheet(id));
        }
    };

    let save_edit = move || {
        let s = summary.get_untracked();
        let title = draft_title.get_untracked();
        let caption = draft_caption.get_untracked();
        ed.send(ClientMessage::UpdateSheet(SheetInfo {
            id: s.info.id,
            title,
            caption,
            enabled: s.info.enabled,
        }));
        mode.set(CardMode::Normal);
    };

    let cancel_edit = move || {
        let s = summary.get_untracked();
        draft_title.set(s.info.title);
        draft_caption.set(s.info.caption);
        mode.set(CardMode::Normal);
    };

    view! {
        <div
            class="sheet-card ctl"
            class:st-disabled=move || summary.with(|s| !s.info.enabled)
            on:click=on_card_click
        >
            {move || {
                // Forms must not rebuild (and lose focus) on every summary update.
                let s = if mode.get() == CardMode::Normal { summary.get() } else { summary.get_untracked() };
                match mode.get() {
                    CardMode::Normal => {
                        let title = s.info.title;
                        let caption = s.info.caption;
                        let enabled = s.info.enabled;
                        let nodes = s.nodes;
                        let errors = s.errors;
                        let card_id = s.info.id;
                        let toggle_title = title.clone();
                        let toggle_caption = caption.clone();

                        view! {
                            <div class="sheet-card-body">
                                <div class="sheet-card-head">
                                    <span class="sheet-title ellipsis">{title}</span>
                                    <div class="sheet-switch" on:click=|ev| ev.stop_propagation()>
                                        <Switch
                                            checked=Signal::derive(move || enabled)
                                            on_toggle=Callback::new(move |new_enabled| {
                                                ed.send(ClientMessage::UpdateSheet(SheetInfo {
                                                    id: card_id,
                                                    title: toggle_title.clone(),
                                                    caption: toggle_caption.clone(),
                                                    enabled: new_enabled,
                                                }));
                                            })
                                        />
                                        <span class="sheet-switch-label">{move || t!("sheet-enabled")}</span>
                                    </div>
                                    <div class="sheet-card-actions" on:click=|ev| ev.stop_propagation()>
                                        <button
                                            class="icon-btn ctl"
                                            title=move || t!("sheet-edit")
                                            aria-label=move || t!("sheet-edit")
                                            on:click=move |_| mode.set(CardMode::Editing)
                                        >
                                            <span class="mdi mdi-pencil-outline" aria-hidden="true"></span>
                                        </button>
                                        <button
                                            class="icon-btn ctl"
                                            title=move || t!("sheet-duplicate")
                                            aria-label=move || t!("sheet-duplicate")
                                            on:click=move |_| ed.send(ClientMessage::DuplicateSheet(card_id))
                                        >
                                            <span class="mdi mdi-content-copy" aria-hidden="true"></span>
                                        </button>
                                        <button
                                            class="icon-btn ctl"
                                            class:st-disabled=move || !can_up()
                                            disabled=move || !can_up()
                                            title=move || t!("sheet-move-up")
                                            aria-label=move || t!("sheet-move-up")
                                            on:click=move |_| {
                                                let idx = index();
                                                if can_move_up(idx) {
                                                    ed.send(ClientMessage::MoveSheet { id: card_id, index: (idx - 1) as u32 });
                                                }
                                            }
                                        >
                                            <span class="mdi mdi-arrow-up" aria-hidden="true"></span>
                                        </button>
                                        <button
                                            class="icon-btn ctl"
                                            class:st-disabled=move || !can_down()
                                            disabled=move || !can_down()
                                            title=move || t!("sheet-move-down")
                                            aria-label=move || t!("sheet-move-down")
                                            on:click=move |_| {
                                                let idx = index();
                                                if can_move_down(idx, total()) {
                                                    ed.send(ClientMessage::MoveSheet { id: card_id, index: (idx + 1) as u32 });
                                                }
                                            }
                                        >
                                            <span class="mdi mdi-arrow-down" aria-hidden="true"></span>
                                        </button>
                                        <button
                                            class="icon-btn ctl"
                                            title=move || t!("sheet-delete")
                                            aria-label=move || t!("sheet-delete")
                                            on:click=move |_| mode.set(CardMode::Deleting)
                                        >
                                            <span class="mdi mdi-delete-outline" aria-hidden="true"></span>
                                        </button>
                                    </div>
                                </div>
                                <div class="sheet-caption">
                                    {if caption.trim().is_empty() {
                                        view! { <span class="sheet-no-caption">{move || t!("sheet-no-caption")}</span> }.into_any()
                                    } else {
                                        view! { <span class="sheet-caption-text">{caption}</span> }.into_any()
                                    }}
                                </div>
                                <div class="sheet-stats">
                                    <span class="sheet-stat">{move || t!("sheet-nodes", count = nodes)}</span>
                                    {(errors > 0).then(|| view! {
                                        <span class="chip st-error">{move || t!("sheet-errors", count = errors)}</span>
                                    })}
                                    <span class="sheet-stat">{move || last_run()}</span>
                                </div>
                            </div>
                        }.into_any()
                    }

                    CardMode::Editing => view! {
                        <div class="sheet-card-edit" on:click=|ev| ev.stop_propagation()>
                            <input
                                type="text"
                                class="form-input sheet-input-title"
                                prop:value=move || draft_title.get()
                                on:input=move |ev| draft_title.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" {
                                        ev.prevent_default();
                                        save_edit();
                                    } else if ev.key() == "Escape" {
                                        ev.prevent_default();
                                        cancel_edit();
                                    }
                                }
                            />
                            <textarea
                                class="form-input form-textarea sheet-input-caption"
                                prop:value=move || draft_caption.get()
                                on:input=move |ev| draft_caption.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Escape" {
                                        ev.prevent_default();
                                        cancel_edit();
                                    }
                                }
                            ></textarea>
                            <div class="sheet-card-edit-actions">
                                <button class="btn ctl" on:click=move |_| save_edit()>{move || t!("sheet-save")}</button>
                                <button class="btn ctl" on:click=move |_| cancel_edit()>{move || t!("sheet-cancel")}</button>
                            </div>
                        </div>
                    }.into_any(),

                    CardMode::Deleting => {
                        let confirm_title = s.info.title;
                        let delete_id = s.info.id;
                        view! {
                            <div class="sheet-card-delete" on:click=|ev| ev.stop_propagation()>
                                <div class="sheet-delete-confirm-text">
                                    {move || t!("sheet-delete-confirm", title = confirm_title.clone())}
                                </div>
                                <div class="sheet-card-edit-actions">
                                    <button
                                        class="btn danger ctl"
                                        on:click=move |_| ed.send(ClientMessage::DeleteSheet(delete_id))
                                    >
                                        {move || t!("sheet-delete")}
                                    </button>
                                    <button class="btn ctl" on:click=move |_| mode.set(CardMode::Normal)>
                                        {move || t!("sheet-cancel")}
                                    </button>
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}
