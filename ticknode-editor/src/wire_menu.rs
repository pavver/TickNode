//! Fixed-position node suggestions for a connection dropped on empty canvas.

use std::collections::HashMap;
use leptos::prelude::*;
use ticknode_shared::{ClientMessage, ConnectionEnd, PropertyOption};

use crate::i18n::{self, t};
use crate::picker::{MAX_ROWS, filter_options};
use crate::node_menu::{MenuRow, MenuRows, NodeMenuPopup, dismiss_menu};
use crate::state::{WireMenu, use_editor};
use crate::wire_suggest::{arrange, suggestions};

#[component]
pub fn WireMenuLayer() -> impl IntoView {
    let ed = use_editor();
    dismiss_menu(".wire-menu", Callback::new(move |_| {
        if ed.wire_menu.get_untracked().is_some() { ed.wire_menu.set(None); }
    }));
    move || ed.wire_menu.get().map(|state| view! { <WireMenuPopup state=state /> })
}

#[component]
fn WireMenuPopup(state: WireMenu) -> impl IntoView {
    let ed = use_editor();
    let origin = Memo::new(move |_| {
        ed.node_tracked(state.from.node).and_then(|n| n.instance.with(|i| {
            if i.unavailable.is_some() { return None; }
            i.definition.pins.iter().find(|p| p.id == state.from.pin && p.direction == state.from.dir)
                .map(|p| p.pin_type.clone())
        }))
    });
    Effect::new(move |_| {
        if origin.get().is_none() { ed.wire_menu.set(None); }
    });
    let choices = Memo::new(move |_| {
        ed.locale.track();
        let Some(kind) = origin.get() else { return Vec::new() };
        let list = ed.definitions.with(|catalog| ed.types.with(|types|
            suggestions(state.from.dir, &kind, catalog, types,
                |d| (i18n::node_title(d), crate::categories::category_label(&d.category)))));
        let usage = ed.nodes.with_untracked(|nodes| {
            let mut usage = HashMap::<String, usize>::new();
            for node in nodes.values() {
                *usage.entry(node.instance.with_untracked(|i| i.model.type_name.clone())).or_default() += 1;
            }
            usage
        });
        ed.recent_nodes.with(|recent| arrange(list, &usage, recent, crate::categories::category_rank))
    });
    let rows = Callback::new(move |query: String| {
        let choices = choices.get();
        let options: Vec<_> = choices.iter().map(|(_, s)| PropertyOption {
            value: s.type_name.clone(), label: s.title.clone(), detail: s.category.clone(), icon: None,
        }).collect();
        let (shown, total) = filter_options(&options, &query, MAX_ROWS);
        MenuRows { total, shown: shown.into_iter().map(|index| {
            let (popular, s) = &choices[index];
            MenuRow {
                index, title: s.title.clone(), detail: s.category.clone(), category: Some(s.group.clone()),
                section: Some(if *popular { t!("wire-menu-popular") } else { s.category.clone() }),
            }
        }).collect() }
    });
    let pick = move |index: usize| {
        if let Some((_, chosen)) = choices.with_untracked(|c| c.get(index).cloned()) {
            ed.send(ClientMessage::AddNodeConnected {
                type_name: chosen.type_name, position: state.position,
                from: ConnectionEnd { node: state.from.node, pin: state.from.pin }, pin: chosen.pin,
            });
            ed.wire_menu.set(None);
        }
    };
    view! {
        <NodeMenuPopup screen=state.screen class="picker-popup wire-menu" id_prefix="wire-menu-opt"
            rows=rows pick=Callback::new(pick) close=Callback::new(move |_| ed.wire_menu.set(None))
            empty_text=Signal::derive(move || t!("wire-menu-empty")) />
    }
}
