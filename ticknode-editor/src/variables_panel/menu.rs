//! Action menu for dropping or clicking a variable to place configured nodes on the canvas.

use crate::canvas::view_center;
use crate::categories::category_label;
use crate::geometry::DU;
use crate::i18n::t;
use crate::node_menu::{MenuRow, MenuRows, NodeMenuPopup, dismiss_menu};
use crate::state::{Editor, use_editor};
use leptos::prelude::*;
use ticknode_shared::{
    ClientMessage, DataValue, Position, Variable, VariableScope, data_type_name,
};

#[derive(Debug, Clone, PartialEq)]
pub struct VariableMenu {
    pub position: Position,
    pub screen: (f64, f64),
    pub variable: Variable,
}

struct ActionDef {
    title_key: &'static str,
    type_name: &'static str,
}

const ACTIONS: [ActionDef; 4] = [
    ActionDef {
        title_key: "variables-action-get",
        type_name: "variable/get",
    },
    ActionDef {
        title_key: "variables-action-set",
        type_name: "variable/set",
    },
    ActionDef {
        title_key: "variables-action-changed",
        type_name: "variable/changed",
    },
    ActionDef {
        title_key: "variables-action-delete",
        type_name: "variable/delete",
    },
];

pub fn open_variable_menu(ed: Editor, variable: Variable, position: Position, screen: (f64, f64)) {
    ed.quick_add.set(None);
    ed.wire_menu.set(None);
    ed.picker.set(None);
    ed.resource_menu.set(None);
    ed.variable_menu.set(Some(VariableMenu {
        position,
        screen,
        variable,
    }));
}

pub fn handle_variable_drop(
    ed: Editor,
    variable: &Variable,
    position: Position,
    screen: (f64, f64),
) {
    open_variable_menu(ed, variable.clone(), position, screen);
}

pub fn handle_variable_click(ed: Editor, variable: &Variable, screen: (f64, f64)) {
    let (cx, cy) = view_center(ed);
    let shift = (ed.nodes.with_untracked(|n| n.len()) % 6) as f64 * 4.0 * DU;
    let position = Position {
        x: (cx - 12.0 * DU + shift) as f32,
        y: (cy - 10.0 * DU + shift) as f32,
    };
    open_variable_menu(ed, variable.clone(), position, screen);
}

#[component]
pub fn VariableMenuLayer() -> impl IntoView {
    let ed = use_editor();
    dismiss_menu(
        ".variable-menu",
        Callback::new(move |_| {
            if ed.variable_menu.get_untracked().is_some() {
                ed.variable_menu.set(None);
            }
        }),
    );
    Effect::new(move |_| {
        if ed.pending_paste.with(|p| p.is_some()) {
            ed.variable_menu.set(None);
        }
    });
    move || {
        ed.variable_menu
            .get()
            .map(|state| view! { <VariableMenuPopup state=state /> })
    }
}

#[component]
fn VariableMenuPopup(state: VariableMenu) -> impl IntoView {
    let ed = use_editor();
    let choices: Vec<(String, &'static str)> = ACTIONS
        .iter()
        .map(|a| {
            let title = match a.title_key {
                "variables-action-get" => t!("variables-action-get"),
                "variables-action-set" => t!("variables-action-set"),
                "variables-action-changed" => t!("variables-action-changed"),
                "variables-action-delete" => t!("variables-action-delete"),
                _ => a.type_name.to_string(),
            };
            (title, a.type_name)
        })
        .collect();

    let choices_for_filter = choices.clone();
    let rows = Callback::new(move |query: String| {
        let q = query.trim().to_lowercase();
        let cat_label = category_label("Variables");
        let filtered: Vec<(usize, &(String, &'static str))> = choices_for_filter
            .iter()
            .enumerate()
            .filter(|(_, (title, type_name))| {
                if q.is_empty() {
                    true
                } else {
                    let haystack = format!("{title} {type_name} {cat_label}").to_lowercase();
                    q.split_whitespace().all(|term| haystack.contains(term))
                }
            })
            .collect();
        let total = filtered.len();
        MenuRows {
            total,
            shown: filtered
                .into_iter()
                .map(|(index, (title, type_name))| MenuRow {
                    index,
                    title: title.clone(),
                    detail: (*type_name).to_string(),
                    category: Some("Variables".to_string()),
                    section: None,
                })
                .collect(),
        }
    });

    let var_data = state.variable.clone();
    let pos = state.position;
    let pick = Callback::new(move |index: usize| {
        if let Some((_, type_name)) = choices.get(index) {
            let lookup = match var_data.key.scope {
                VariableScope::Sheet => "sheet",
                VariableScope::Global | VariableScope::Persistent => "global",
            };
            let properties = if *type_name == "variable/delete" {
                vec![
                    (
                        "name".to_string(),
                        DataValue::String(var_data.key.name.clone()),
                    ),
                    ("lookup".to_string(), DataValue::String(lookup.to_string())),
                ]
            } else {
                vec![
                    (
                        "name".to_string(),
                        DataValue::String(var_data.key.name.clone()),
                    ),
                    ("lookup".to_string(), DataValue::String(lookup.to_string())),
                    (
                        "value_type".to_string(),
                        DataValue::String(data_type_name(&var_data.data_type)),
                    ),
                ]
            };
            ed.send(ClientMessage::AddNodeWith {
                type_name: (*type_name).to_string(),
                position: pos,
                properties,
            });
            if ed.narrow.get_untracked() {
                ed.show_palette.set(false);
            }
            ed.variable_menu.set(None);
        }
    });

    view! {
        <NodeMenuPopup
            screen=state.screen
            class="picker-popup variable-menu"
            id_prefix="variable-menu-opt"
            rows=rows
            pick=pick
            close=Callback::new(move |_| ed.variable_menu.set(None))
            empty_text=Signal::derive(move || t!("picker-empty"))
        />
    }
}
