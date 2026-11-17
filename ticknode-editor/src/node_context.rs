//! Context menu of nodes on the canvas: duplicate, copy and delete the selection.

use leptos::prelude::*;
use operator_ui::prelude::{Menu, MenuAnchor, MenuItem, icons};

use crate::clipboard;
use crate::i18n::t;
use crate::state::{LogKind, Selection, use_editor};

#[component]
pub fn NodeContextLayer() -> impl IntoView {
    let ed = use_editor();
    move || {
        ed.node_menu.get().map(|(x, y)| {
            let open = RwSignal::new(true);
            Effect::new(move |_| {
                if !open.get() {
                    ed.node_menu.set(None);
                }
            });
            let count = ed.selection.with_untracked(|s| match s {
                Some(Selection::Nodes(ids)) => ids.len(),
                _ => 0,
            });
            let items = vec![
                MenuItem::new("duplicate", t!("node-menu-duplicate"))
                    .icon(icons::ADD)
                    .shortcut("Ctrl+D"),
                MenuItem::new("copy", t!("node-menu-copy"))
                    .icon(icons::CONTENT_COPY)
                    .shortcut("Ctrl+C"),
                MenuItem {
                    separator: true,
                    ..MenuItem::new("delete", t!("node-menu-delete", count = count))
                        .icon(icons::DELETE)
                        .shortcut("Del")
                        .danger()
                },
            ];
            let select = Callback::new(move |id: String| {
                ed.node_menu.set(None);
                match id.as_str() {
                    "duplicate" => clipboard::duplicate(ed),
                    "copy" => {
                        if !clipboard::copy_via_command() {
                            ed.log(
                                LogKind::Error,
                                "[ERROR] Browser blocked copying - try Ctrl+C",
                            );
                        }
                    }
                    "delete" => clipboard::delete_selection(ed),
                    _ => {}
                }
            });
            view! {
                <Menu
                    open=open
                    anchor=Signal::stored(MenuAnchor::Point(x, y))
                    items=items
                    on_select=select
                    label=t!("node-menu-label")
                />
            }
        })
    }
}
