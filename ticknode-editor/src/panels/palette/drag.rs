use leptos::prelude::*;
use ticknode_shared::*;
use web_sys::PointerEvent;

use super::PALETTE_DRAG_THRESHOLD;
use crate::canvas::{CANVAS_ID, view_center};
use crate::geometry::DU;
use crate::i18n;
use crate::state::{Editor, PaletteDrag, use_editor};

pub fn move_palette_drag(ed: Editor, ev: &PointerEvent) {
    ed.palette_drag.update(|drag| {
        if let Some(drag) = drag {
            drag.update_motion(
                (ev.client_x() as f64, ev.client_y() as f64),
                PALETTE_DRAG_THRESHOLD,
            );
        }
    });
}

/// Spawns the node where the pointer was released (cursor positioned over the node header).
pub fn drop_on_canvas(ed: Editor, drag: &PaletteDrag, ev: &PointerEvent) {
    let (x, y) = (ev.client_x(), ev.client_y());
    let over_canvas = document()
        .element_from_point(x as f32, y as f32)
        .and_then(|el| el.closest(&format!("#{CANVAS_ID}")).ok().flatten());
    let Some(canvas) = over_canvas else { return };
    let r = canvas.get_bounding_client_rect();
    let (wx, wy) = ed
        .view
        .get_untracked()
        .to_world(x as f64 - r.left(), y as f64 - r.top());
    let snap = |v: f64| ((v / DU).round() * DU) as f32;
    let position = Position {
        x: snap(wx - 8.0 * DU),
        y: snap(wy - 4.0 * DU),
    };
    match drag {
        PaletteDrag::Node { type_name, .. } => {
            ed.send(ClientMessage::AddNode {
                type_name: type_name.clone(),
                position,
            });
            if ed.narrow.get_untracked() {
                ed.show_palette.set(false);
            }
        }
        PaletteDrag::Resource {
            plugin,
            collection_id,
            item,
            actions,
            ..
        } => {
            handle_resource_drop(
                ed,
                plugin,
                collection_id,
                item,
                actions,
                position,
                (x as f64, y as f64),
            );
        }
        PaletteDrag::Variable { variable, .. } => {
            crate::variables_panel::menu::handle_variable_drop(
                ed,
                variable,
                position,
                (x as f64, y as f64),
            );
        }
    }
}

pub fn handle_resource_drop(
    ed: Editor,
    plugin: &str,
    collection_id: &str,
    item: &PropertyOption,
    actions: &[ResourceAction],
    position: Position,
    screen: (f64, f64),
) {
    if actions.len() == 1 {
        let action = &actions[0];
        ed.send(ClientMessage::AddNodeWith {
            type_name: action.type_name.clone(),
            position,
            properties: vec![(
                action.property.clone(),
                DataValue::String(item.value.clone()),
            )],
        });
        if ed.narrow.get_untracked() {
            ed.show_palette.set(false);
        }
    } else if actions.len() > 1 {
        open_resource_menu(ed, plugin, collection_id, item, actions, position, screen);
    }
}

pub fn handle_resource_click(
    ed: Editor,
    plugin: &str,
    collection_id: &str,
    item: &PropertyOption,
    actions: &[ResourceAction],
    screen: (f64, f64),
) {
    let (cx, cy) = view_center(ed);
    let shift = (ed.nodes.with_untracked(|n| n.len()) % 6) as f64 * 4.0 * DU;
    let position = Position {
        x: (cx - 12.0 * DU + shift) as f32,
        y: (cy - 10.0 * DU + shift) as f32,
    };
    handle_resource_drop(ed, plugin, collection_id, item, actions, position, screen);
}

pub fn open_resource_menu(
    ed: Editor,
    plugin: &str,
    collection_id: &str,
    item: &PropertyOption,
    actions: &[ResourceAction],
    position: Position,
    screen: (f64, f64),
) {
    let choices: Vec<crate::resource_menu::ResourceActionChoice> = actions
        .iter()
        .map(|a| {
            let title = i18n::resource_action_title(plugin, collection_id, a);
            let category = ed.definitions.with_untracked(|defs| {
                defs.iter()
                    .find(|d| d.type_name == a.type_name)
                    .map(|d| d.category.clone())
            });
            crate::resource_menu::ResourceActionChoice {
                title,
                type_name: a.type_name.clone(),
                property: a.property.clone(),
                category,
            }
        })
        .collect();
    ed.quick_add.set(None);
    ed.wire_menu.set(None);
    ed.picker.set(None);
    ed.variable_menu.set(None);
    ed.resource_menu
        .set(Some(crate::resource_menu::ResourceMenu {
            position,
            screen,
            choices,
            item_value: item.value.clone(),
        }));
}

/// Ghost node under the cursor while dragging from the palette or resources.
#[component]
pub fn PaletteGhost() -> impl IntoView {
    let ed = use_editor();
    move || {
        ed.palette_drag.get().filter(|d| d.active()).map(|d| view! {
            <div class="palette-ghost" style=format!("left:{}px;top:{}px;--cat-color:{}", d.at().0, d.at().1, d.color())>
                <span class="node-title ellipsis">{d.title().to_string()}</span>
            </div>
        })
    }
}
