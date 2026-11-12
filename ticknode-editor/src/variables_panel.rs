//! Shared variables list component used on sheet pages (left panel tab) and the home page.

mod create_form;
pub mod logic;
pub mod menu;
mod row;
mod value_editor;

use crate::i18n::t;
use crate::state::use_editor;
use crate::ui::{TextField, icons};
use create_form::CreateVariableForm;
use leptos::prelude::*;
use logic::group_and_filter_variables;
use row::VariableRow;
use ticknode_shared::SheetId;

#[component]
pub fn VariablesList(sheet: Option<SheetId>) -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let is_creating = RwSignal::new(false);

    let all_vars = Memo::new(move |_| {
        ed.variables
            .with(|vars| vars.values().cloned().collect::<Vec<_>>())
    });

    let total_visible_count = Memo::new(move |_| {
        let vars = all_vars.get();
        vars.iter()
            .filter(|v| match v.key.scope {
                ticknode_shared::VariableScope::Sheet => sheet.is_some() && v.key.sheet == sheet,
                ticknode_shared::VariableScope::Global
                | ticknode_shared::VariableScope::Persistent => true,
            })
            .count()
    });

    let groups = Memo::new(move |_| {
        let vars = all_vars.get();
        let q = query.get();
        ed.types
            .with(|types| group_and_filter_variables(&vars, sheet, &q, types))
    });

    let is_home = sheet.is_none();

    view! {
        <div class="variables-panel" class:home-mode=is_home>
            <div class="variables-toolbar">
                <div class="variables-search">
                    <TextField
                        value=Signal::derive(String::new)
                        placeholder=Signal::derive(move || t!("variables-search-placeholder"))
                        icon=icons::SEARCH
                        on_input=Callback::new(move |v| query.set(v))
                    />
                </div>
                <button
                    class="btn primary ctl btn-sm var-new-btn"
                    title=move || t!("variables-new")
                    on:click=move |_| is_creating.update(|c| *c = !*c)
                >
                    <span class="mdi mdi-plus" aria-hidden="true"></span>
                    <span>{move || t!("variables-new")}</span>
                </button>
            </div>

            {move || is_creating.get().then(|| view! {
                <CreateVariableForm sheet=sheet on_close=Callback::new(move |_| is_creating.set(false)) />
            })}

            <div class="variables-list">
                {move || {
                    let total = total_visible_count.get();
                    if total == 0 {
                        return view! {
                            <div class="empty variables-empty">{move || t!("variables-empty")}</div>
                        }.into_any();
                    }

                    let cur_groups = groups.get();
                    if cur_groups.is_empty() {
                        return view! {
                            <div class="empty variables-no-match">{move || t!("variables-no-match")}</div>
                        }.into_any();
                    }

                    view! {
                        <div class="variables-groups">
                            {(!cur_groups.sheet.is_empty()).then(|| {
                                let items = cur_groups.sheet.clone();
                                let count = items.len();
                                view! {
                                    <div class="var-group">
                                        <div class="var-group-header">
                                            <span class="var-group-title">{move || t!("variables-scope-sheet")}</span>
                                            <span class="var-group-count">{count}</span>
                                        </div>
                                        <For each=move || items.clone() key=|i| i.variable.key.clone() let:item>
                                            <VariableRow item=item can_drag=true />
                                        </For>
                                    </div>
                                }
                            })}

                            {(!cur_groups.global.is_empty()).then(|| {
                                let items = cur_groups.global.clone();
                                let count = items.len();
                                view! {
                                    <div class="var-group">
                                        <div class="var-group-header">
                                            <span class="var-group-title">{move || t!("variables-scope-global")}</span>
                                            <span class="var-group-count">{count}</span>
                                        </div>
                                        <For each=move || items.clone() key=|i| i.variable.key.clone() let:item>
                                            <VariableRow item=item can_drag=sheet.is_some() />
                                        </For>
                                    </div>
                                }
                            })}

                            {(!cur_groups.persistent.is_empty()).then(|| {
                                let items = cur_groups.persistent.clone();
                                let count = items.len();
                                view! {
                                    <div class="var-group">
                                        <div class="var-group-header">
                                            <span class="var-group-title">{move || t!("variables-scope-persistent")}</span>
                                            <span class="var-group-count">{count}</span>
                                        </div>
                                        <For each=move || items.clone() key=|i| i.variable.key.clone() let:item>
                                            <VariableRow item=item can_drag=sheet.is_some() />
                                        </For>
                                    </div>
                                }
                            })}
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}
