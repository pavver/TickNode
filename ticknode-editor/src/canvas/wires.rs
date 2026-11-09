//! Connections and the wire currently being dragged.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::*;
use crate::state::{Editor, Selection, use_editor};
use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WireEnd {
    pub(crate) node: NodeId,
    pub(crate) pin: PinId,
    pub(crate) dir: PinDirection,
    pub(crate) exec: bool,
}

#[derive(Clone, Copy)]
pub(super) struct PendingConnection(pub(super) RwSignal<Option<WireEnd>>);

pub(super) fn connection_pin(ed: Editor, node: NodeId, pin: PinId, dir: PinDirection) -> Option<PinType> {
    ed.node_tracked(node)?.instance.with(|i| pin_type(&i.definition, pin, dir).cloned())
}

fn anchor(id: NodeId, pin: PinId, dir: PinDirection) -> Option<(f64, f64)> {
    let ed = use_editor();
    let node = ed.node_tracked(id)?;
    let pos = node.pos.get();
    node.instance.with(|i| NodeLayout::of(i).anchor(pos, pin, dir))
}

#[component]
pub(super) fn Wire(conn: ConnectionModel, exec: bool) -> impl IntoView {
    let ed = use_editor();
    let path = Memo::new(move |_| {
        let a = anchor(conn.src_node, conn.src_pin, PinDirection::Output)?;
        let b = anchor(conn.dst_node, conn.dst_pin, PinDirection::Input)?;
        Some(wire_path(a, b))
    });
    let color = Memo::new(move |_| {
        ed.node_tracked(conn.src_node)
            .and_then(|n| n.instance.with(|i| pin_type(&i.definition, conn.src_pin, PinDirection::Output).map(|t| ed.types.with(|types| pin_color(types, t)))))
            .unwrap_or_else(|| "var(--border-strong)".to_string())
    });
    let conversion = Memo::new(move |_| {
        let src = connection_pin(ed, conn.src_node, conn.src_pin, PinDirection::Output)?;
        let dst = connection_pin(ed, conn.dst_node, conn.dst_pin, PinDirection::Input)?;
        ed.types.with(|types| wire_conversion(types, &src, &dst)).filter(|converts| *converts)?;
        match (src, dst) {
            (PinType::Data(src), PinType::Data(dst)) => Some(t!("wire-conversion", src = ed.type_title(&src), dst = ed.type_title(&dst))),
            _ => None,
        }
    });
    let selection = if exec { Selection::Exec(conn) } else { Selection::Data(conn) };
    let selected = Memo::new(move |_| ed.selection.with(|s| s.as_ref() == Some(&selection)));
    move || {
        path.get().map(|d| {
            view! {
                <g>
                    <title>{move || conversion.get().unwrap_or_default()}</title>
                    <path
                        class="wire"
                        class:converting=move || conversion.get().is_some()
                        class:exec=exec
                        class:selected=selected
                        style=move || if exec || selected.get() { String::new() } else { format!("stroke:{}", color.get()) }
                        d=d.clone()
                    />
                    <path
                        class="wire-hit"
                        d=d
                        data-wire=if exec { "exec" } else { "data" }
                        data-src=conn.src_node.0.to_string()
                        data-src-pin=conn.src_pin.0.to_string()
                        data-dst=conn.dst_node.0.to_string()
                        data-dst-pin=conn.dst_pin.0.to_string()
                    />
                </g>
            }
        })
    }
}

#[component]
pub(super) fn PendingWire(from: RwSignal<Option<WireEnd>>, cursor: RwSignal<(f64, f64)>) -> impl IntoView {
    let ed = use_editor();
    move || {
        let menu = ed.wire_menu.get();
        let end = from.get().or_else(|| menu.map(|m| m.from))?;
        let a = anchor(end.node, end.pin, end.dir)?;
        let c = menu.map(|m| (m.position.x as f64, m.position.y as f64)).unwrap_or_else(|| cursor.get());
        let d = if end.dir == PinDirection::Output { wire_path(a, c) } else { wire_path(c, a) };
        Some(view! { <path class="wire pending" d=d /> })
    }
}
