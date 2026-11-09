//! Paste placement and the fragment preview.

use std::collections::HashMap;

use leptos::prelude::*;
use ticknode_shared::*;

use crate::categories::category_label;
use crate::clipboard;
use crate::geometry::*;
use crate::i18n;
use crate::state::{Editor, use_editor};

/// Moves the pending paste fragment along with the cursor (`p` is in canvas coordinates).
pub(super) fn move_paste(ed: Editor, p: (f64, f64)) {
    if ed.pending_paste.with_untracked(|p| p.is_none()) {
        return;
    }
    let at = clipboard::snap_point(ed.view.get_untracked().to_world(p.0, p.1));
    ed.pending_paste.update(|pending| {
        if let Some(pending) = pending.as_mut().filter(|pending| pending.at != at) {
            pending.at = at;
        }
    });
}

/// Translucent "ghost" of the fragment being pasted. Definitions are looked up in the registry by type —
/// for nodes whose pins depend on properties, the preview may differ slightly from the final result.
#[component]
pub(super) fn PastePreview() -> impl IntoView {
    let ed = use_editor();
    move || {
        let pending = ed.pending_paste.get()?;
        let (ax, ay) = pending.at;
        let defs = ed.definitions.with(|defs| {
            pending.graph.nodes.iter()
                .map(|n| (n.id, defs.iter().find(|d| d.type_name == n.type_name).cloned()))
                .collect::<HashMap<_, _>>()
        });
        let pos = |n: &NodeModel| Position { x: n.position.x + ax as f32, y: n.position.y + ay as f32 };
        let positions: HashMap<NodeId, Position> = pending.graph.nodes.iter().map(|n| (n.id, pos(n))).collect();
        let layouts: HashMap<NodeId, NodeLayout> = pending.graph.nodes.iter()
            .filter_map(|n| Some((n.id, NodeLayout::sized(defs.get(&n.id)?.as_ref()?, &n.properties, n.size, &pending.graph.data_connections.iter().filter(|c| c.dst_node == n.id).map(|c| c.dst_pin).collect::<Vec<_>>()))))
            .collect();
        let wire = |c: &ConnectionModel| {
            let a = layouts.get(&c.src_node)?.anchor(*positions.get(&c.src_node)?, c.src_pin, PinDirection::Output)?;
            let b = layouts.get(&c.dst_node)?.anchor(*positions.get(&c.dst_node)?, c.dst_pin, PinDirection::Input)?;
            Some(wire_path(a, b))
        };
        let wires = pending.graph.exec_connections.iter().chain(&pending.graph.data_connections)
            .filter_map(wire)
            .map(|d| view! { <path class="wire pending" d=d /> })
            .collect_view();
        let types = ed.types.get();
        let nodes = pending.graph.nodes.iter().map(|n| {
            let p = positions[&n.id];
            let def = defs.get(&n.id).cloned().flatten();
            // Ghost renders only header and pins (no fields), but matches the real node's size
            let stored = n.size.and_then(NodeSize::validated);
            let height = layouts.get(&n.id).map_or(stored.map_or(MIN_NODE_HEIGHT, |s| s.height.max(MIN_NODE_HEIGHT)), |l| l.height);
            let width = layouts.get(&n.id).map_or(stored.map_or(node_width(&n.type_name), |s| s.width), |l| l.width);
            let top = layouts.get(&n.id).map_or(0.0, |l| l.pins_top);
            let (title, category) = def.as_ref().map_or((n.type_name.clone(), String::new()), |d| (i18n::node_title(d), category_label(&d.category)));
            let layout = layouts.get(&n.id).cloned();
            let pin_list = |dir: PinDirection| def.as_ref().map(|d| pins(d, dir).map(|pin| {
                let exec = pin.pin_type == PinType::Exec;
                // Position comes from the layout (in Match, outputs sit next to their respective cases)
                let y = layout.as_ref().and_then(|l| l.anchor(Position::default(), pin.id, dir)).map_or(0.0, |(_, y)| y - top);
                let side = if dir == PinDirection::Input { "left:0" } else { "right:0" };
                let pin_label = i18n::node_pin_label(&d.type_name, &pin.name);
                view! {
                    <div class=if dir == PinDirection::Input { "pin in" } else { "pin out" } style=format!("position:absolute;{side};top:{}px;white-space:nowrap", y - ROW_H / 2.0)>
                        <span class="socket" class:exec=exec style=format!("--pin-color:{}", pin_color(&types, &pin.pin_type))></span>
                        <span class="ellipsis">{pin_label}</span>
                    </div>
                }
            }).collect_view());
            view! {
                <div class="node paste-ghost" style=format!("left:{}px;top:{}px;width:{}px;height:{}px", p.x, p.y, width, height)>
                    <div class="node-header">
                        <span class="node-title ellipsis">{title}</span>
                        <span class="node-cat ellipsis">{category}</span>
                    </div>
                    <div class="pins in" style=format!("top:{top}px")>{pin_list(PinDirection::Input)}</div>
                    <div class="pins out" style=format!("top:{top}px")>{pin_list(PinDirection::Output)}</div>
                </div>
            }
        }).collect_view();
        Some(view! {
            <svg class="wires paste-ghost">{wires}</svg>
            {nodes}
        })
    }
}
