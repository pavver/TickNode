//! Connections and the wire currently being dragged.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::*;
use crate::i18n::t;
use crate::state::{Editor, Selection, use_editor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WireEnd {
    pub(crate) node: NodeId,
    pub(crate) pin: PinId,
    pub(crate) dir: PinDirection,
    pub(crate) exec: bool,
}

#[derive(Clone, Copy)]
pub(super) struct PendingConnection(pub(super) RwSignal<Option<WireEnd>>);

pub(super) fn connection_pin(
    ed: Editor,
    node: NodeId,
    pin: PinId,
    dir: PinDirection,
) -> Option<PinType> {
    ed.node_tracked(node)?
        .instance
        .with(|i| pin_type(&i.definition, pin, dir).cloned())
}

pub(crate) fn anchor(ed: Editor, id: NodeId, pin: PinId, dir: PinDirection) -> Option<(f64, f64)> {
    let node = ed.node_tracked(id)?;
    let pos = node.pos.get();
    ed.layout(id)?.anchor(pos, pin, dir)
}

#[component]
pub(super) fn Wire(conn: ConnectionModel, exec: bool) -> impl IntoView {
    let ed = use_editor();
    let route = Memo::new(move |_| {
        ed.wire_routes
            .with(|all| all.get(&(exec, conn)).cloned().unwrap_or_default())
    });
    let path = Memo::new(move |_| {
        let a = anchor(ed, conn.src_node, conn.src_pin, PinDirection::Output)?;
        let b = anchor(ed, conn.dst_node, conn.dst_pin, PinDirection::Input)?;
        Some(route_path(a, &route.get(), b))
    });
    let midpoint = Memo::new(move |_| {
        let a = anchor(ed, conn.src_node, conn.src_pin, PinDirection::Output)?;
        let b = anchor(ed, conn.dst_node, conn.dst_pin, PinDirection::Input)?;
        Some(route_midpoint(a, &route.get(), b))
    });
    let color = Memo::new(move |_| {
        ed.node_tracked(conn.src_node)
            .and_then(|n| {
                n.instance.with(|i| {
                    pin_type(&i.definition, conn.src_pin, PinDirection::Output)
                        .map(|t| ed.types.with(|types| pin_color(types, t)))
                })
            })
            .unwrap_or_else(|| "var(--border-strong)".to_string())
    });
    let conversion = Memo::new(move |_| {
        let src = connection_pin(ed, conn.src_node, conn.src_pin, PinDirection::Output)?;
        let dst = connection_pin(ed, conn.dst_node, conn.dst_pin, PinDirection::Input)?;
        ed.types
            .with(|types| wire_conversion(types, &src, &dst))
            .filter(|converts| *converts)?;
        match (src, dst) {
            (PinType::Data(src), PinType::Data(dst)) => Some(t!(
                "wire-conversion",
                src = ed.type_title(&src),
                dst = ed.type_title(&dst)
            )),
            _ => None,
        }
    });
    let selection = if exec {
        Selection::Exec(conn)
    } else {
        Selection::Data(conn)
    };
    let selected = Memo::new(move |_| ed.selection.with(|s| s.as_ref() == Some(&selection)));
    move || {
        path.get().map(|d| {
            view! {
                <g class=move || if selected.get() { "wire-group selected" } else { "wire-group" }
                    style=move || format!("--wire-color:{}", if exec { "var(--text-secondary)".to_string() } else { color.get() })
                    on:pointerenter=move |ev| crate::node_status::capture_title(&ev, || {
                        let value = crate::node_status::pin_status(ed, conn.src_node, conn.src_pin, PinDirection::Output, exec);
                        match conversion.get() { Some(conversion) => format!("{conversion}\n{value}"), None => value }
                    })>
                    <title></title>
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
                    {move || {
                        if exec || !ed.show_values.get() { return None; }
                        let value = crate::node_status::wire_value(ed, conn)?;
                        let (x, y) = midpoint.get()?;
                        let width = value.chars().count() as f64 * 7.0 + 16.0;
                        Some(view! {
                            <g class="wire-value" transform=format!("translate({x},{y})")
                                on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
                                on:pointerenter=move |ev| crate::node_status::capture_title(&ev, || crate::node_status::pin_status(ed, conn.src_node, conn.src_pin, PinDirection::Output, false))>
                                <title></title>
                                <rect x=-width / 2.0 y=-10 width=width height=20 />
                                <text text-anchor="middle" dominant-baseline="central">{value}</text>
                            </g>
                        })
                    }}
                    {move || route.get().into_iter().enumerate().map(|(index, point)| view! {
                        <rect class=move || if ed.selected_point.get() == Some(((exec, conn), index)) { "wire-point current" } else { "wire-point" }
                            x=point.x as f64 - DU y=point.y as f64 - DU width=2.0 * DU height=2.0 * DU
                            data-wire-point=index.to_string()
                            data-wire=if exec { "exec" } else { "data" }
                            data-src=conn.src_node.0.to_string() data-src-pin=conn.src_pin.0.to_string()
                            data-dst=conn.dst_node.0.to_string() data-dst-pin=conn.dst_pin.0.to_string()
                        />
                    }).collect_view()}
                </g>
            }
        })
    }
}

#[component]
pub(super) fn PendingWire(
    from: RwSignal<Option<WireEnd>>,
    cursor: RwSignal<(f64, f64)>,
) -> impl IntoView {
    let ed = use_editor();
    move || {
        let menu = ed.wire_menu.get();
        let end = from.get().or_else(|| menu.map(|m| m.from))?;
        let a = anchor(ed, end.node, end.pin, end.dir)?;
        let c = menu
            .map(|m| (m.position.x as f64, m.position.y as f64))
            .unwrap_or_else(|| cursor.get());
        let d = if end.dir == PinDirection::Output {
            wire_path(a, c)
        } else {
            wire_path(c, a)
        };
        Some(view! { <path class="wire pending" d=d /> })
    }
}
