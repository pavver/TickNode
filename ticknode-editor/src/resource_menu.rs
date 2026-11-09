//! Action choice popup for dropping or clicking a resource with multiple available actions.

use leptos::prelude::*;
use ticknode_shared::{ClientMessage, DataValue, Position};
use crate::categories::category_label;
use crate::i18n::t;
use crate::node_menu::{MenuRow, MenuRows, NodeMenuPopup, dismiss_menu};
use crate::state::use_editor;

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceActionChoice {
    pub title: String,
    pub type_name: String,
    pub property: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceMenu {
    pub position: Position,
    pub screen: (f64, f64),
    pub choices: Vec<ResourceActionChoice>,
    pub item_value: String,
}

#[component]
pub fn ResourceMenuLayer() -> impl IntoView {
    let ed = use_editor();
    dismiss_menu(".resource-menu", Callback::new(move |_| {
        if ed.resource_menu.get_untracked().is_some() {
            ed.resource_menu.set(None);
        }
    }));
    Effect::new(move |_| {
        if ed.pending_paste.with(|p| p.is_some()) {
            ed.resource_menu.set(None);
        }
    });
    move || ed.resource_menu.get().map(|state| view! { <ResourceMenuPopup state=state /> })
}

#[component]
fn ResourceMenuPopup(state: ResourceMenu) -> impl IntoView {
    let ed = use_editor();
    let choices = state.choices.clone();
    let rows = Callback::new(move |query: String| {
        let q = query.trim().to_lowercase();
        let filtered: Vec<(usize, &ResourceActionChoice)> = choices
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if q.is_empty() {
                    true
                } else {
                    let cat_label = c.category.as_deref().map(category_label).unwrap_or_default();
                    let haystack = format!("{} {} {}", c.title, c.type_name, cat_label).to_lowercase();
                    q.split_whitespace().all(|term| haystack.contains(term))
                }
            })
            .collect();
        let total = filtered.len();
        MenuRows {
            total,
            shown: filtered
                .into_iter()
                .map(|(index, c)| MenuRow {
                    index,
                    title: c.title.clone(),
                    detail: c.type_name.clone(),
                    category: c.category.clone(),
                    section: None,
                })
                .collect(),
        }
    });
    let pick = Callback::new(move |index: usize| {
        if let Some(choice) = state.choices.get(index) {
            ed.send(ClientMessage::AddNodeWith {
                type_name: choice.type_name.clone(),
                position: state.position,
                properties: vec![(choice.property.clone(), DataValue::String(state.item_value.clone()))],
            });
            if ed.narrow.get_untracked() {
                ed.show_palette.set(false);
            }
            ed.resource_menu.set(None);
        }
    });
    view! {
        <NodeMenuPopup
            screen=state.screen
            class="picker-popup resource-menu"
            id_prefix="resource-menu-opt"
            rows=rows
            pick=pick
            close=Callback::new(move |_| ed.resource_menu.set(None))
            empty_text=Signal::derive(move || t!("picker-empty"))
        />
    }
}
