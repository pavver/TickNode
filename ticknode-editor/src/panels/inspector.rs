//! Inspectors for nodes, selections and connections.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::{self, t};
use crate::state::use_editor;
use crate::categories::category_label;
use crate::clipboard;
use crate::geometry::type_color;
use crate::state::{LogKind, Selection};
use crate::ui::*;
use super::execution::ExecutionInfo;
use super::formatting::instance_pin_label;
use super::tooltip::InfoButton;


// --- Inspector ---

#[component]
pub fn Inspector() -> impl IntoView {
    let ed = use_editor();
    // The plugins panel takes the inspector's slot
    let hidden = move || ed.show_plugins.get() || (ed.narrow.get() && ed.selection.get().is_none());
    view! {
        <aside class="panel right" class:hidden=hidden>
            {move || match ed.selection.get() {
                Some(Selection::Nodes(ids)) if ids.len() == 1 => {
                    let id = *ids.first().expect("one node");
                    view! { <NodeInspector id=id /> }.into_any()
                }
                Some(Selection::Nodes(ids)) => view! { <GroupInspector count=ids.len() /> }.into_any(),
                Some(Selection::Exec(c)) => view! { <ConnectionInspector conn=c exec=true /> }.into_any(),
                Some(Selection::Data(c)) => view! { <ConnectionInspector conn=c exec=false /> }.into_any(),
                Some(Selection::Detached(d)) => view! {
                    <div class="panel-section">
                        <span class="t-body-small">{move || t!("wire-detached-title")}</span>
                        <ActionButton text=Signal::derive(move || t!("wire-detached-remove")) icon=icons::DELETE
                            danger=true full=true on_click=Callback::new(move |_| ed.send(ClientMessage::RemoveDetached(d))) />
                    </div>
                }.into_any(),
                Some(Selection::Frame(_)) | None => view! { <GraphOverview /> }.into_any(),
            }}
        </aside>
    }
}

#[component]
fn GraphOverview() -> impl IntoView {
    let ed = use_editor();
    let stats = move || {
        let nodes = ed.nodes.with(|n| n.len());
        let conns = ed.exec.with(|e| e.len()) + ed.data.with(|d| d.len());
        t!("overview-stats", nodes = nodes, connections = conns)
    };
    view! {
        <div class="section-title">{move || t!("overview-title-graph")}</div>
        <div class="panel-section"><span class="t-row">{stats}</span></div>
        <div class="section-title">{move || t!("overview-title-controls")}</div>
        <div class="panel-section">
            <span class="t-body-small">{move || t!("overview-hint-1")}</span>
            <span class="t-body-small">{move || t!("overview-hint-2")}</span>
            <span class="t-body-small">{move || t!("overview-hint-3")}</span>
            <span class="t-body-small">{move || t!("overview-hint-4")}</span>
            <span class="t-body-small">{move || t!("overview-hint-5")}</span>
            <span class="t-body-small">{move || t!("overview-hint-6")}</span>
            <span class="t-body-small">{move || t!("overview-hint-7")}</span>
            <span class="t-body-small">{move || t!("overview-hint-8")}</span>
            <span class="t-body-small">{move || t!("overview-hint-9")}</span>
            <span class="t-body-small">{move || t!("overview-hint-resize")}</span>
            <span class="t-body-small">{move || t!("overview-hint-quick-add")}</span>
            <span class="t-body-small">{move || t!("overview-hint-frame")}</span>
        </div>
    }
}

#[component]
fn NodeInspector(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else {
        return view! { <div class="empty">{move || t!("inspector-node-not-found")}</div> }.into_any();
    };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));

    view! {
        <div class="panel-section">
            <div class="inspector-head">
                <span class="t-row ellipsis">{move || def.with(i18n::node_title)}</span>
                {move || view! { <InfoButton def=def.get() /> }}
            </div>
            <span class="t-caption" title=move || t!("inspector-cost-help")>{move || def.with(|d| format!("{} · {} · {}", category_label(&d.category), d.type_name, t!("inspector-cost", cost = d.cost)))}</span>
            <span class="t-body-small">{move || def.with(i18n::node_description)}</span>
        </div>
        // Properties, input values, conditions, and triggers live directly on the node
        <crate::frozen::FrozenInfo id=id />
        <ExecutionInfo id=id />
        <div class="section-title">{move || t!("inspector-section-actions")}</div>
        <div class="panel-section">
            <ActionButton
                text=Signal::derive(move || t!("inspector-duplicate"))
                icon=icons::DUPLICATE
                full=true
                on_click=Callback::new(move |_| clipboard::duplicate(ed))
            />
            <ActionButton
                text=Signal::derive(move || t!("inspector-delete-node"))
                icon=icons::DELETE
                danger=true
                full=true
                on_click=Callback::new(move |_| ed.send(ClientMessage::RemoveNode(id)))
            />
        </div>
    }
    .into_any()
}

#[component]
fn GroupInspector(count: usize) -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="panel-section">
            <span class="t-row">{move || t!("group-selected", count = count)}</span>
            <span class="t-body-small">{move || t!("group-hint")}</span>
        </div>
        <div class="section-title">{move || t!("inspector-section-actions")}</div>
        <div class="panel-section">
            <ActionButton
                text=Signal::derive(move || t!("group-copy"))
                icon=icons::COPY
                full=true
                flash_on_click=true
                on_click=Callback::new(move |_| if !clipboard::copy_via_command() {
                    ed.log(LogKind::Error, "[ERROR] Browser blocked copying - try Ctrl+C");
                })
            />
            <ActionButton
                text=Signal::derive(move || t!("inspector-duplicate"))
                icon=icons::DUPLICATE
                full=true
                on_click=Callback::new(move |_| clipboard::duplicate(ed))
            />
            <ActionButton
                text=Signal::derive(move || t!("group-delete", count = count))
                icon=icons::DELETE
                danger=true
                full=true
                on_click=Callback::new(move |_| clipboard::delete_selection(ed))
            />
        </div>
    }
}

#[component]
fn ConnectionInspector(conn: ConnectionModel, exec: bool) -> impl IntoView {
    let ed = use_editor();
    let pin_name = move |node: NodeId, pin: PinId, dir: PinDirection| {
        ed.node_tracked(node)
            .and_then(|n| {
                n.instance.with(|i| {
                    i.definition
                        .pins
                        .iter()
                        .find(|p| p.id == pin && p.direction == dir)
                        .map(|p| instance_pin_label(i, pin, dir, &p.name))
                })
            })
            .unwrap_or_else(|| format!("#{}", pin.0))
    };
    let from = move || format!("{} · {}", ed.node_title(conn.src_node), pin_name(conn.src_node, conn.src_pin, PinDirection::Output));
    let to = move || format!("{} · {}", ed.node_title(conn.dst_node), pin_name(conn.dst_node, conn.dst_pin, PinDirection::Input));
    let color = if exec {
        "var(--text-primary)".to_string()
    } else {
        ed.node(conn.src_node)
            .and_then(|n| n.instance.with_untracked(|i| match crate::geometry::pin_type(&i.definition, conn.src_pin, PinDirection::Output) {
                Some(PinType::Data(t)) => Some(ed.types.with_untracked(|types| type_color(types, t))),
                _ => None,
            }))
            .unwrap_or_else(|| "var(--border-strong)".to_string())
    };
    let disconnect = move |_| {
        ed.send(if exec {
            ClientMessage::DisconnectExec(conn)
        } else {
            ClientMessage::DisconnectData { dst_node: conn.dst_node, dst_pin: conn.dst_pin }
        })
    };
    view! {
        <div class="panel-section">
            <span class="t-row">{move || if exec { t!("conn-type-exec") } else { t!("conn-type-data") }}</span>
        </div>
        <div class="panel-section">
            <div class="dense-row"><span class="socket connected" style=format!("--pin-color:{color}")></span><span class="t-caption">{move || t!("conn-from")}</span></div>
            <span class="t-body-small">{from}</span>
            <div class="dense-row"><span class="socket connected" style=format!("--pin-color:{color}")></span><span class="t-caption">{move || t!("conn-to")}</span></div>
            <span class="t-body-small">{to}</span>
        </div>
        <div class="panel-section">
            <ActionButton
                text=Signal::derive(move || t!("conn-disconnect"))
                icon=icons::DELETE
                danger=true
                full=true
                on_click=Callback::new(disconnect)
            />
        </div>
    }
}
