//! Полотно редактора: ноди (DOM) і дроти (SVG) у спільному трансформованому шарі.

use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent, WheelEvent};

use crate::geometry::*;
use crate::categories::{category_color, category_label};
use crate::clipboard;
use crate::inline::{NodeProps, PinValue};
use crate::match_editor::MatchBody;
use crate::ui::{Icon, icons};
use crate::state::{EXEC_HIGHLIGHT_MS, Editor, LogKind, Selection, use_editor};

pub const CANVAS_ID: &str = "ticknode-canvas";
/// Зсув курсора (px), після якого натискання вважається перетягуванням, а не кліком.
const DRAG_THRESHOLD: f64 = 3.0;
const GRID: f64 = 5.0 * DU;

#[derive(Debug, Clone, Copy, PartialEq)]
struct WireEnd {
    node: NodeId,
    pin: PinId,
    dir: PinDirection,
    exec: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Idle,
    Pan { start: (f64, f64), origin: Camera, moved: bool },
    /// Перетягування виділених нод; `clicked` — нода під курсором.
    Nodes { origins: Vec<(NodeId, Position)>, start_world: (f64, f64), start: (f64, f64), moved: bool, clicked: NodeId },
    Wire { from: WireEnd },
    Pinch { start_dist: f64, start_mid: (f64, f64), origin: Camera },
    /// Рамка виділення (екранні координати полотна). `additive` — Shift/Ctrl: додати до виділення.
    Box { start: (f64, f64), additive: bool },
}

/// Рамка виділення як у CAD: зліва направо — «вікно» (нода має бути всередині повністю),
/// справа наліво — «перетин» (достатньо торкнутись).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SelectBox {
    start: (f64, f64),
    end: (f64, f64),
}

impl SelectBox {
    fn crossing(&self) -> bool {
        self.end.0 < self.start.0
    }

    fn rect(&self) -> (f64, f64, f64, f64) {
        (self.start.0.min(self.end.0), self.start.1.min(self.end.1), self.start.0.max(self.end.0), self.start.1.max(self.end.1))
    }
}

/// Ноди, які захоплює рамка (у світових координатах).
fn nodes_in_box(ed: Editor, b: SelectBox) -> Vec<NodeId> {
    let view = ed.view.get_untracked();
    let (x0, y0, x1, y1) = b.rect();
    let (wx0, wy0) = view.to_world(x0, y0);
    let (wx1, wy1) = view.to_world(x1, y1);
    let crossing = b.crossing();
    ed.nodes.with_untracked(|nodes| {
        nodes.iter().filter(|(_, n)| {
            let p = n.pos.get_untracked();
            let (w, h) = n.instance.with_untracked(instance_size);
            let (nx0, ny0, nx1, ny1) = (p.x as f64, p.y as f64, p.x as f64 + w, p.y as f64 + h);
            if crossing {
                nx0 <= wx1 && nx1 >= wx0 && ny0 <= wy1 && ny1 >= wy0
            } else {
                nx0 >= wx0 && nx1 <= wx1 && ny0 >= wy0 && ny1 <= wy1
            }
        }).map(|(id, _)| *id).collect()
    })
}

fn additive(ev: &PointerEvent) -> bool {
    ev.shift_key() || ev.ctrl_key() || ev.meta_key()
}

fn canvas_element() -> Option<Element> {
    document().get_element_by_id(CANVAS_ID)
}

/// Центр видимої області у світових координатах.
pub fn view_center(ed: Editor) -> (f64, f64) {
    let rect = canvas_element().map(|c| c.get_bounding_client_rect());
    let (w, h) = rect.map(|r| (r.width(), r.height())).unwrap_or((800.0, 600.0));
    ed.view.get_untracked().to_world(w / 2.0, h / 2.0)
}

/// Масштаб і зсув, за яких видно всі ноди.
pub fn fit_view(ed: Editor) {
    let Some(rect) = canvas_element().map(|c| c.get_bounding_client_rect()) else { return };
    let bounds = ed.nodes.with_untracked(|nodes| {
        nodes.values().fold(None, |acc: Option<(f64, f64, f64, f64)>, n| {
            let p = n.pos.get_untracked();
            let (w, h) = n.instance.with_untracked(instance_size);
            let (x0, y0, x1, y1) = (p.x as f64, p.y as f64, p.x as f64 + w, p.y as f64 + h);
            Some(match acc {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            })
        })
    });
    let Some((x0, y0, x1, y1)) = bounds else {
        ed.view.set(Camera::default());
        return;
    };
    let pad = 10.0 * DU;
    let (bw, bh) = (x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad);
    let zoom = (rect.width() / bw).min(rect.height() / bh).clamp(MIN_ZOOM, 1.0);
    ed.view.set(Camera {
        x: (rect.width() - (x1 - x0) * zoom) / 2.0 - x0 * zoom,
        y: (rect.height() - (y1 - y0) * zoom) / 2.0 - y0 * zoom,
        zoom,
    });
}

fn parse_node_id(s: &str) -> Option<NodeId> {
    uuid::Uuid::parse_str(s).ok().map(NodeId)
}

fn closest(target: Option<web_sys::EventTarget>, selector: &str) -> Option<Element> {
    target?.dyn_into::<Element>().ok()?.closest(selector).ok().flatten()
}

fn parse_pin(el: &Element) -> Option<WireEnd> {
    Some(WireEnd {
        node: parse_node_id(&el.get_attribute("data-node")?)?,
        pin: PinId(el.get_attribute("data-pin")?.parse().ok()?),
        dir: if el.get_attribute("data-dir")? == "in" { PinDirection::Input } else { PinDirection::Output },
        exec: el.get_attribute("data-kind")? == "exec",
    })
}

fn parse_wire(el: &Element) -> Option<Selection> {
    let c = ConnectionModel {
        src_node: parse_node_id(&el.get_attribute("data-src")?)?,
        src_pin: PinId(el.get_attribute("data-src-pin")?.parse().ok()?),
        dst_node: parse_node_id(&el.get_attribute("data-dst")?)?,
        dst_pin: PinId(el.get_attribute("data-dst-pin")?.parse().ok()?),
    };
    Some(if el.get_attribute("data-wire")? == "exec" { Selection::Exec(c) } else { Selection::Data(c) })
}

/// Фрагмент, що вставляється, їде за курсором (`p` — координати полотна).
fn move_paste(ed: Editor, p: (f64, f64)) {
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

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[component]
pub fn Canvas() -> impl IntoView {
    let ed = use_editor();
    let canvas = NodeRef::<html::Div>::new();
    let drag = StoredValue::new(Drag::Idle);
    let panning = RwSignal::new(false);
    let cursor = RwSignal::new((0.0, 0.0));
    let pending_wire = RwSignal::new(None::<WireEnd>);
    let select_box = RwSignal::new(None::<SelectBox>);
    let pointers = StoredValue::new(HashMap::<i32, (f64, f64)>::new());

    let local_point = move |ev: &PointerEvent| -> (f64, f64) {
        canvas
            .get_untracked()
            .map(|c| {
                let r = c.get_bounding_client_rect();
                (ev.client_x() as f64 - r.left(), ev.client_y() as f64 - r.top())
            })
            .unwrap_or((0.0, 0.0))
    };

    // Нові позиції перетягнутих нод — на сервер. Позицію вже змінено локально, тож перерване
    // перетягування (pointercancel, другий палець) теж надсилається, інакше редактор і сервер розійдуться.
    let send_moves = move |origins: Vec<(NodeId, Position)>| {
        for (id, origin) in origins {
            if let Some(node) = ed.node(id) {
                let position = node.pos.get_untracked();
                if position != origin {
                    ed.send(ClientMessage::MoveNode { id, position });
                }
            }
        }
    };

    let finish = move |ev: &PointerEvent, commit: bool| {
        pointers.update_value(|p| {
            p.remove(&ev.pointer_id());
        });
        let state = drag.get_value();
        drag.set_value(Drag::Idle);
        panning.set(false);
        pending_wire.set(None);
        let boxed = select_box.get_untracked();
        select_box.set(None);
        if let Drag::Nodes { origins, moved: true, .. } = state {
            send_moves(origins);
            return;
        }
        if !commit {
            return;
        }
        match state {
            Drag::Pan { moved: false, .. } => ed.selection.set(None),
            // Клік без перетягування по ноді з групи — виділяємо лише її.
            Drag::Nodes { moved: false, clicked, .. } => ed.select_nodes([clicked]),
            Drag::Box { start, additive } => {
                let b = boxed.unwrap_or(SelectBox { start, end: start });
                if distance(b.start, b.end) <= DRAG_THRESHOLD {
                    if !additive {
                        ed.selection.set(None);
                    }
                    return;
                }
                let mut ids = nodes_in_box(ed, b);
                if additive {
                    ids.extend(ed.selected_nodes());
                }
                ed.select_nodes(ids);
            }
            Drag::Wire { from } => {
                let hit = document().element_from_point(ev.client_x() as f32, ev.client_y() as f32);
                let Some(to) = hit.and_then(|el| el.closest("[data-pin]").ok().flatten()).and_then(|el| parse_pin(&el)) else {
                    return;
                };
                if to.node == from.node && to.pin == from.pin {
                    return;
                }
                if to.dir == from.dir {
                    ed.log(LogKind::Error, "[ERROR] З'єднувати можна лише вихід із входом");
                    return;
                }
                if to.exec != from.exec {
                    ed.log(LogKind::Error, "[ERROR] Exec-пін з'єднується лише з exec-піном");
                    return;
                }
                let (out, inp) = if from.dir == PinDirection::Output { (from, to) } else { (to, from) };
                let c = ConnectionModel { src_node: out.node, src_pin: out.pin, dst_node: inp.node, dst_pin: inp.pin };
                ed.send(if from.exec { ClientMessage::ConnectExec(c) } else { ClientMessage::ConnectData(c) });
            }
            _ => {}
        }
    };

    let on_pointer_down = move |ev: PointerEvent| {
        let p = local_point(&ev);
        // Розміщення вставки: ЛКМ (або дотик) ставить, ПКМ скасовує, середня — зсув як завжди
        if ed.pending_paste.with_untracked(|p| p.is_some()) && ev.button() != 1 {
            match ev.button() {
                0 => {
                    move_paste(ed, p);
                    clipboard::place_paste(ed);
                }
                2 => { clipboard::cancel_paste(ed); }
                _ => {}
            }
            return;
        }
        if ev.pointer_type() == "mouse" && ev.button() != 0 && ev.button() != 1 {
            return;
        }
        pointers.update_value(|ps| {
            ps.insert(ev.pointer_id(), p);
        });
        if let Some(c) = canvas.get_untracked() {
            let _ = c.set_pointer_capture(ev.pointer_id());
        }

        // Другий палець — масштабування жестом
        let active: Vec<(f64, f64)> = pointers.with_value(|ps| ps.values().copied().collect());
        if active.len() == 2 {
            if let Drag::Nodes { origins, moved: true, .. } = drag.get_value() {
                send_moves(origins);
            }
            pending_wire.set(None);
            drag.set_value(Drag::Pinch {
                start_dist: distance(active[0], active[1]).max(1.0),
                start_mid: ((active[0].0 + active[1].0) / 2.0, (active[0].1 + active[1].1) / 2.0),
                origin: ed.view.get_untracked(),
            });
            return;
        }

        let view = ed.view.get_untracked();
        let world = view.to_world(p.0, p.1);
        let target = ev.target();
        if ev.button() == 1 {
            // Середня кнопка — зсув полотна (як у CAD), звідки б не почали
            ev.prevent_default();
            drag.set_value(Drag::Pan { start: p, origin: view, moved: true });
            panning.set(true);
        } else if let Some(end) = closest(target.clone(), "[data-pin]").and_then(|el| parse_pin(&el)) {
            let mut from = end;
            // Тягнемо від підключеного входу даних — від'єднуємо і переносимо дріт
            if end.dir == PinDirection::Input && !end.exec {
                let existing = ed.data.with_untracked(|d| d.iter().find(|c| c.dst_node == end.node && c.dst_pin == end.pin).copied());
                if let Some(c) = existing {
                    ed.send(ClientMessage::DisconnectData { dst_node: c.dst_node, dst_pin: c.dst_pin });
                    from = WireEnd { node: c.src_node, pin: c.src_pin, dir: PinDirection::Output, exec: false };
                }
            }
            cursor.set(world);
            pending_wire.set(Some(from));
            drag.set_value(Drag::Wire { from });
        } else if let Some(sel) = closest(target.clone(), "[data-wire]").and_then(|el| parse_wire(&el)) {
            ed.selection.set(Some(sel));
        } else if let Some(id) = closest(target, "[data-node]").and_then(|el| el.get_attribute("data-node")).and_then(|s| parse_node_id(&s)) {
            if additive(&ev) {
                // Shift/Ctrl+клік — додати ноду до виділення або прибрати з нього
                let mut ids = ed.selected_nodes();
                match ids.iter().position(|n| *n == id) {
                    Some(i) => { ids.remove(i); }
                    None => ids.push(id),
                }
                ed.select_nodes(ids);
                return;
            }
            if !ed.selection.with_untracked(|s| matches!(s, Some(Selection::Nodes(set)) if set.contains(&id))) {
                ed.select_nodes([id]);
            }
            let origins = ed.selected_nodes().into_iter()
                .filter_map(|n| ed.node(n).map(|node| (n, node.pos.get_untracked())))
                .collect();
            drag.set_value(Drag::Nodes { origins, start_world: world, start: p, moved: false, clicked: id });
        } else if ev.pointer_type() == "touch" {
            // На сенсорному екрані палець по фону зсуває полотно
            drag.set_value(Drag::Pan { start: p, origin: view, moved: false });
            panning.set(true);
        } else {
            drag.set_value(Drag::Box { start: p, additive: additive(&ev) });
        }
    };

    let on_pointer_move = move |ev: PointerEvent| {
        let p = local_point(&ev);
        pointers.update_value(|ps| {
            if let Some(v) = ps.get_mut(&ev.pointer_id()) {
                *v = p;
            }
        });
        ed.pointer_world.set_value(Some(ed.view.get_untracked().to_world(p.0, p.1)));
        move_paste(ed, p);
        match drag.get_value() {
            Drag::Idle => {}
            Drag::Box { start, .. } => select_box.set(Some(SelectBox { start, end: p })),
            Drag::Pan { start, origin, moved } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                drag.set_value(Drag::Pan { start, origin, moved });
                ed.view.set(Camera { x: origin.x + p.0 - start.0, y: origin.y + p.1 - start.1, ..origin });
            }
            Drag::Nodes { origins, start_world, start, moved, clicked } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                if moved {
                    let (wx, wy) = ed.view.get_untracked().to_world(p.0, p.1);
                    let (dx, dy) = (wx - start_world.0, wy - start_world.1);
                    // Прив'язка до сітки 1 DU
                    let snap = |v: f64| ((v / DU).round() * DU) as f32;
                    for (id, origin) in &origins {
                        if let Some(node) = ed.node(*id) {
                            node.pos.set(Position { x: snap(origin.x as f64 + dx), y: snap(origin.y as f64 + dy) });
                        }
                    }
                }
                drag.set_value(Drag::Nodes { origins, start_world, start, moved, clicked });
            }
            Drag::Wire { .. } => cursor.set(ed.view.get_untracked().to_world(p.0, p.1)),
            Drag::Pinch { start_dist, start_mid, origin } => {
                let active: Vec<(f64, f64)> = pointers.with_value(|ps| ps.values().copied().collect());
                if active.len() == 2 {
                    let mid = ((active[0].0 + active[1].0) / 2.0, (active[0].1 + active[1].1) / 2.0);
                    let zoom = (origin.zoom * distance(active[0], active[1]) / start_dist).clamp(MIN_ZOOM, MAX_ZOOM);
                    let (wx, wy) = origin.to_world(start_mid.0, start_mid.1);
                    ed.view.set(Camera { x: mid.0 - wx * zoom, y: mid.1 - wy * zoom, zoom });
                }
            }
        }
    };

    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let Some(c) = canvas.get_untracked() else { return };
        let r = c.get_bounding_client_rect();
        let line = if ev.delta_mode() == 1 { 16.0 } else { 1.0 };
        let factor = (-ev.delta_y() * line * 0.0015).exp();
        let (sx, sy) = (ev.client_x() as f64 - r.left(), ev.client_y() as f64 - r.top());
        ed.view.update(|v| *v = v.zoom_at(sx, sy, factor));
    };

    let grid_style = move || {
        let v = ed.view.get();
        let size = GRID * v.zoom;
        format!("background-size:{size}px {size}px;background-position:{}px {}px", v.x, v.y)
    };
    let viewport_style = move || {
        let v = ed.view.get();
        format!("transform:translate({}px,{}px) scale({})", v.x, v.y, v.zoom)
    };
    let node_ids = move || ed.nodes.with(|n| n.keys().copied().collect::<Vec<_>>());

    view! {
        <div
            id=CANVAS_ID
            class="canvas"
            class:panning=move || panning.get()
            class:placing=move || ed.pending_paste.with(|p| p.is_some())
            node_ref=canvas
            style=grid_style
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=move |ev| finish(&ev, true)
            on:pointercancel=move |ev| finish(&ev, false)
            on:pointerleave=move |_| ed.pointer_world.set_value(None)
            // Інакше браузер вмикає автопрокрутку на середню кнопку
            on:mousedown=|ev: web_sys::MouseEvent| if ev.button() == 1 { ev.prevent_default() }
            on:wheel=on_wheel
            on:contextmenu=|ev| ev.prevent_default()
        >
            <div
                class="viewport"
                class:multi-select=move || ed.selection.with(|s| matches!(s, Some(Selection::Nodes(set)) if set.len() > 1))
                style=viewport_style
            >
                <svg class="wires">
                    <For each=move || ed.exec.get() key=|c| *c let:c>
                        <Wire conn=c exec=true />
                    </For>
                    <For each=move || ed.data.get() key=|c| *c let:c>
                        <Wire conn=c exec=false />
                    </For>
                    <PendingWire from=pending_wire cursor=cursor />
                </svg>
                <SelectionBounds />
                <For each=node_ids key=|id| *id let:id>
                    <NodeView id=id />
                </For>
                <PastePreview />
            </div>
            <Show when=move || ed.nodes.with(|n| n.is_empty())>
                <div class="canvas-hint">
                    <div class="t-section">"Порожній граф"</div>
                    <div class="t-caption">"Додайте ноду з палітри й з'єднайте піни перетягуванням"</div>
                </div>
            </Show>
            {move || select_box.get().map(|b| {
                let (x0, y0, x1, y1) = b.rect();
                view! {
                    <div
                        class="select-box"
                        class:crossing=b.crossing()
                        style=format!("left:{x0}px;top:{y0}px;width:{}px;height:{}px", x1 - x0, y1 - y0)
                    ></div>
                }
            })}
            <div class="zoom-badge">{move || format!("{:.0}%", ed.view.get().zoom * 100.0)}</div>
        </div>
    }
}

/// Відступ рамки виділення від нод (у світових координатах). Групова рамка ширша,
/// щоб не зливатися з рамками окремих нод (`.multi-select .node.selected::before`).
const BOUNDS_PAD: f64 = 3.0 * DU;
const GROUP_BOUNDS_PAD: f64 = 5.0 * DU;

/// Пунктирна рамка навколо виділених нод; для групи — з кількістю нод.
#[component]
fn SelectionBounds() -> impl IntoView {
    let ed = use_editor();
    let bounds = move || {
        let ids = ed.selection.with(|s| match s {
            Some(Selection::Nodes(set)) => set.iter().copied().collect::<Vec<_>>(),
            _ => Vec::new(),
        });
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut count = 0;
        for id in ids {
            let Some(node) = ed.node_tracked(id) else { continue };
            let p = node.pos.get();
            let (w, h) = node.instance.with(instance_size);
            x0 = x0.min(p.x as f64);
            y0 = y0.min(p.y as f64);
            x1 = x1.max(p.x as f64 + w);
            y1 = y1.max(p.y as f64 + h);
            count += 1;
        }
        let pad = if count > 1 { GROUP_BOUNDS_PAD } else { BOUNDS_PAD };
        (count > 0).then_some((x0 - pad, y0 - pad, x1 + pad, y1 + pad, count))
    };
    move || bounds().map(|(x0, y0, x1, y1, count)| view! {
        <div
            class="selection-bounds"
            style=format!("left:{x0}px;top:{y0}px;width:{}px;height:{}px", x1 - x0, y1 - y0)
        >
            {(count > 1).then(|| view! { <span class="selection-count">{count}</span> })}
        </div>
    })
}

/// Напівпрозорий «привид» фрагмента, що вставляється. Дефініції беруться з реєстру за типом —
/// для нод, чиї піни залежать від властивостей, вигляд може трохи відрізнятися від результату.
#[component]
fn PastePreview() -> impl IntoView {
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
            .filter_map(|n| Some((n.id, NodeLayout::new(defs.get(&n.id)?.as_ref()?, &n.properties))))
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
            // Привид — лише заголовок і піни (без полів), але розміром як справжня нода
            let height = layouts.get(&n.id).map_or(MIN_NODE_HEIGHT, |l| l.height);
            let top = layouts.get(&n.id).map_or(0.0, |l| l.pins_top);
            let (title, category) = def.as_ref().map_or((n.type_name.clone(), String::new()), |d| (d.title.clone(), category_label(&d.category)));
            let layout = layouts.get(&n.id).cloned();
            let pin_list = |dir: PinDirection| def.as_ref().map(|d| pins(d, dir).map(|pin| {
                let exec = pin.pin_type == PinType::Exec;
                // Позиція — з розкладки (у Match виходи стоять біля своїх випадків)
                let y = layout.as_ref().and_then(|l| l.anchor(Position::default(), pin.id, dir)).map_or(0.0, |(_, y)| y - top);
                let side = if dir == PinDirection::Input { "left:0" } else { "right:0" };
                view! {
                    <div class=if dir == PinDirection::Input { "pin in" } else { "pin out" } style=format!("position:absolute;{side};top:{}px;white-space:nowrap", y - ROW_H / 2.0)>
                        <span class="socket" class:exec=exec style=format!("--pin-color:{}", pin_color(&types, &pin.pin_type))></span>
                        <span class="ellipsis">{pin.name.clone()}</span>
                    </div>
                }
            }).collect_view());
            view! {
                <div class="node paste-ghost" style=format!("left:{}px;top:{}px;width:{}px;height:{}px", p.x, p.y, node_width(&n.type_name), height)>
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

fn anchor(id: NodeId, pin: PinId, dir: PinDirection) -> Option<(f64, f64)> {
    let ed = use_editor();
    let node = ed.node_tracked(id)?;
    let pos = node.pos.get();
    node.instance.with(|i| NodeLayout::of(i).anchor(pos, pin, dir))
}

#[component]
fn Wire(conn: ConnectionModel, exec: bool) -> impl IntoView {
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
    let selection = if exec { Selection::Exec(conn) } else { Selection::Data(conn) };
    let selected = Memo::new(move |_| ed.selection.with(|s| s.as_ref() == Some(&selection)));
    move || {
        path.get().map(|d| {
            view! {
                <g>
                    <path
                        class="wire"
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
fn PendingWire(from: RwSignal<Option<WireEnd>>, cursor: RwSignal<(f64, f64)>) -> impl IntoView {
    move || {
        let end = from.get()?;
        let a = anchor(end.node, end.pin, end.dir)?;
        let c = cursor.get();
        let d = if end.dir == PinDirection::Output { wire_path(a, c) } else { wire_path(c, a) };
        Some(view! { <path class="wire pending" d=d /> })
    }
}

/// Пін, з якого нода запускається вручну: перший exec-вхід; для джерел подій (Clock, кнопка) —
/// їхній exec-вихід.
pub fn trigger_pin(def: &NodeDefinition) -> Option<PinId> {
    let exec = |dir| def.pins.iter().find(|p| p.direction == dir && p.pin_type == PinType::Exec).map(|p| p.id);
    exec(PinDirection::Input).or_else(|| exec(PinDirection::Output))
}

#[component]
fn NodeView(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.nodes.with_untracked(|n| n.get(&id).copied()) else {
        return ().into_any();
    };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));
    let layout = Memo::new(move |_| node.instance.with(NodeLayout::of));
    let is_match = move || def.with(|d| d.type_name == "match");
    let style = move || {
        let p = node.pos.get();
        def.with(|d| format!(
            "left:{}px;top:{}px;width:{}px;height:{}px;--cat-color:{}",
            p.x, p.y, layout.with(|l| l.width), layout.with(|l| l.height), category_color(&d.category)
        ))
    };
    let pins_style = move || format!("top:{}px", layout.with(|l| l.pins_top));
    let pin_list = move |dir: PinDirection| {
        def.with(|d| pins(d, dir).cloned().collect::<Vec<_>>())
            .into_iter()
            .map(|p| view! { <Pin node=id pin=p dir=dir /> })
            .collect_view()
    };
    // Нода-заглушка: плагін її типу недоступний — нода приглушена, властивості не редагуються
    let unavailable = Memo::new(move |_| node.instance.with(|i| i.unavailable.clone()));
    let trigger = Memo::new(move |_| if unavailable.with(Option::is_some) { None } else { def.with(trigger_pin) });
    view! {
        <div
            class="node"
            class:unavailable=move || unavailable.with(Option::is_some)
            title=move || unavailable.get().unwrap_or_default()
            data-node=id.0.to_string()
            class:missing=move || def.with(|d| d.pins.iter().any(|p| ed.required_missing(id, p)))
            class:selected=move || ed.is_selected(id)
            class:active=move || ed.now.get() - node.last_exec.get() < EXEC_HIGHLIGHT_MS
            style=style
        >
            <div class="node-header">
                <span class="node-title ellipsis">{move || def.with(|d| d.title.clone())}</span>
                <span class="node-cat ellipsis" class:node-unavailable=move || unavailable.with(Option::is_some)>
                    {move || if unavailable.with(Option::is_some) { "плагін недоступний".to_string() } else { def.with(|d| category_label(&d.category)) }}
                </span>
                {move || def.with(|d| d.pins.iter().any(|p| ed.required_missing(id, p))).then(|| view! {
                    <span class="node-warn" title="Не задано обов'язковий вхід — нода не виконується">"!"</span>
                })}
                {move || trigger.get().map(|pin| view! {
                    // Кнопка — для неї нода й існує, тож «запустити» видно завжди; решті — при наведенні
                    <button
                        class="node-run"
                        class:always=move || def.with(|d| d.type_name == "test/click_button")
                        title="Запустити"
                        on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
                        on:click=move |_| ed.send(ClientMessage::TriggerNode { id, pin })
                    >
                        <Icon path=icons::PLAY />
                    </button>
                })}
            </div>
            <NodeProps id=id />
            // Match малює свої рядки сам: входи з іменем і типом, далі випадки
            {move || if is_match() {
                view! { <MatchBody id=id top=Signal::derive(move || layout.with(|l| l.pins_top)) /> }.into_any()
            } else {
                view! {
                    <div class="pins in" style=pins_style>{move || pin_list(PinDirection::Input)}</div>
                    <div class="pins out" style=pins_style>{move || pin_list(PinDirection::Output)}</div>
                }.into_any()
            }}
        </div>
    }
    .into_any()
}

#[component]
pub fn Pin(
    node: NodeId,
    pin: PinDefinition,
    dir: PinDirection,
    /// Підпис замість імені піна.
    #[prop(optional, into)]
    label: Option<String>,
) -> impl IntoView {
    let ed = use_editor();
    let exec = pin.pin_type == PinType::Exec;
    let pin_id = pin.id;
    let connected = move || match dir {
        PinDirection::Input => ed.is_input_connected(node, pin_id),
        PinDirection::Output => ed.is_output_connected(node, pin_id),
    };
    let type_name = match &pin.pin_type {
        PinType::Exec => "exec".to_string(),
        PinType::Data(t) => ed.type_title(t),
    };
    let color = ed.types.with_untracked(|types| pin_color(types, &pin.pin_type));
    let is_input = dir == PinDirection::Input;
    let pin_def = pin.clone();
    let missing = move || ed.required_missing(node, &pin_def);
    let required_note = if pin.required { " · обов'язковий" } else { "" };
    // Непід'єднаному входу даних можна задати значення прямо тут
    let value = match &pin.pin_type {
        PinType::Data(t) if is_input && ed.types.with_untracked(|types| input_value_editable(types, t)) => {
            let t = t.clone();
            Some(move || (!connected()).then(|| view! { <PinValue ed=ed node=node pin=pin_id value_type=t.clone() /> }))
        }
        _ => None,
    };
    view! {
        <div
            // Увесь клас одним рядком: динамічний `class` перезаписує окремі `class:`
            class=move || match (is_input, missing()) {
                (true, true) => "pin in missing",
                (true, false) => "pin in",
                (false, _) => "pin out",
            }
            title=format!("{}: {}{required_note}", pin.name, type_name)
            data-pin=pin_id.0.to_string()
            data-node=node.0.to_string()
            data-dir=if is_input { "in" } else { "out" }
            data-kind=if exec { "exec" } else { "data" }
        >
            <span class="socket" class:exec=exec class:connected=connected style=format!("--pin-color:{color}")></span>
            <span class="ellipsis">{label.unwrap_or_else(|| pin.name.clone())}</span>
            {value}
        </div>
    }
}
