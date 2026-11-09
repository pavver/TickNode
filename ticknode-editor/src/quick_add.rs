//! Add a catalog node at an empty canvas point using the shared node menu.

use leptos::prelude::*;
use ticknode_shared::{ClientMessage, Position};
use crate::i18n::{self, t};
use crate::node_menu::{MenuRow, MenuRows, NodeMenuPopup, dismiss_menu};
use crate::picker::MAX_ROWS;
use crate::state::use_editor;

mod catalog;
pub mod recent;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuickAdd {
    pub position: Position,
    pub screen: (f64, f64),
}

#[component]
pub fn QuickAddLayer() -> impl IntoView {
    let ed = use_editor();
    dismiss_menu(".quick-add", Callback::new(move |_| {
        if ed.quick_add.get_untracked().is_some() { ed.quick_add.set(None); }
    }));
    Effect::new(move |_| {
        if ed.pending_paste.with(|p| p.is_some()) { ed.quick_add.set(None); }
    });
    move || ed.quick_add.get().map(|state| view! { <QuickAddPopup state=state /> })
}

#[component]
fn QuickAddPopup(state: QuickAdd) -> impl IntoView {
    let ed = use_editor();
    let rows = Callback::new(move |query: String| {
        ed.locale.track();
        let choices = ed.definitions.with(|catalog| ed.recent_nodes.with(|recent| {
            catalog::ranked(catalog, recent, &query, |d| (
                i18n::node_title(d), crate::categories::category_label(&d.category), i18n::node_description(d),
            ))
        }));
        let total = choices.len();
        let has_recent = choices.first().is_some_and(|c| c.recent);
        MenuRows { total, shown: choices.into_iter().take(MAX_ROWS).map(|c| MenuRow {
            section: has_recent.then(|| if c.recent { t!("quick-add-recent") } else { t!("quick-add-all-nodes") }),
            index: c.index, title: c.title, detail: c.category,
            category: ed.definitions.with(|defs| Some(defs[c.index].category.clone())),
        }).collect() }
    });
    let pick = Callback::new(move |index: usize| {
        if let Some(def) = ed.definitions.with_untracked(|defs| defs.get(index).cloned()) {
            ed.send(ClientMessage::AddNode { type_name: def.type_name, position: state.position });
            ed.quick_add.set(None);
        }
    });
    view! {
        <NodeMenuPopup screen=state.screen class="picker-popup quick-add" id_prefix="quick-add-opt"
            rows=rows pick=pick close=Callback::new(move |_| ed.quick_add.set(None))
            empty_text=Signal::derive(move || t!("picker-empty")) />
    }
}
