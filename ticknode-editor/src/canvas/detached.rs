//! Detached wire rendering and endpoint decisions.

use leptos::prelude::*;
use ticknode_shared::*;
use crate::geometry::*;
use crate::i18n::t;
use crate::state::{Selection, use_editor};
use super::wires::WireEnd;

/// Returns the live endpoint first, then the broken endpoint.
pub(super) fn ends(d: DetachedConnection) -> (WireEnd, WireEnd) {
    let c = d.connection;
    let source = WireEnd { node: c.src_node, pin: c.src_pin, dir: PinDirection::Output, exec: d.exec };
    let target = WireEnd { node: c.dst_node, pin: c.dst_pin, dir: PinDirection::Input, exec: d.exec };
    match d.end { DetachedEnd::Source => (target, source), DetachedEnd::Target => (source, target) }
}

/// Do not remove the old wire unless both current pins form a compatible connection.
pub(super) fn reconnect(d: DetachedConnection, to: WireEnd, live: Option<&PinType>, target: Option<&PinType>, types: &TypeRegistry, data: &[ConnectionModel]) -> Option<ClientMessage> {
    let (mut from, _) = ends(d);
    from.exec = matches!(live?, PinType::Exec);
    if from.dir == to.dir || from.exec != to.exec || (from.node == to.node && from.pin == to.pin) { return None; }
    let (a, b) = if from.dir == PinDirection::Output { (live?, target?) } else { (target?, live?) };
    wire_conversion(types, a, b)?;
    let (src, dst) = if from.dir == PinDirection::Output { (from, to) } else { (to, from) };
    let c = ConnectionModel { src_node: src.node, src_pin: src.pin, dst_node: dst.node, dst_pin: dst.pin };
    if !from.exec && data_cycle(c, data) { return None; }
    Some(ClientMessage::Batch(vec![if from.exec { ClientMessage::ConnectExec(c) } else { ClientMessage::ConnectData(c) }, ClientMessage::RemoveDetached(d)]))
}

/// Batch commands permit partial success, so reject data cycles before removing the old wire.
fn data_cycle(c: ConnectionModel, data: &[ConnectionModel]) -> bool {
    let mut stack = vec![c.src_node];
    let mut visited = std::collections::HashSet::new();
    while let Some(node) = stack.pop() {
        if node == c.dst_node { return true; }
        if visited.insert(node) { stack.extend(data.iter().filter(|c| c.dst_node == node).map(|c| c.src_node)); }
    }
    false
}

#[component]
pub(super) fn DetachedWire(detached: DetachedConnection) -> impl IntoView {
    let ed = use_editor();
    let geometry = Memo::new(move |_| {
        let (live, broken) = ends(detached);
        let anchor = |end: WireEnd, loose| {
            let node = ed.node_tracked(end.node)?;
            Some(node.instance.with(|i| NodeLayout::of(i).detached_anchor(node.pos.get(), end.pin, end.dir, detached.row, loose)))
        };
        let a = anchor(live, false)?;
        let b = anchor(broken, true)?;
        Some((if live.dir == PinDirection::Output { wire_path(a, b) } else { wire_path(b, a) }, b))
    });
    let selected = move || ed.selection.with(|s| s.as_ref() == Some(&Selection::Detached(detached)));
    let attr = serde_json::to_string(&detached).expect("detached wire serializes");
    move || geometry.get().map(|(path, (x, y))| view! {
        <g class="detached-group" class:selected=selected>
            <title>{move || t!("wire-detached-title")}</title>
            <path class="wire detached" class:selected=selected d=path.clone() />
            <path class="wire-hit" d=path data-detached=attr.clone() />
            <rect class="detached-square" x=x - DU y=y - DU width=2.0 * DU height=2.0 * DU />
            <rect class="detached-end-hit" x=x - 2.0 * DU y=y - 2.0 * DU width=4.0 * DU height=4.0 * DU
                data-detached=attr.clone() data-detached-end="true" />
        </g>
    })
}

#[cfg(test)]
mod tests;
