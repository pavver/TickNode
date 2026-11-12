//! Node palette, left panel tabs, drag placement and drag preview.

use leptos::html;
use leptos::prelude::*;
use std::collections::BTreeMap;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use super::resources::ResourcesPanel;
use super::tooltip::InfoButton;
use super::variables::VariablesPanel;
use crate::canvas::view_center;
use crate::categories::{category_color, category_icon, category_label, category_rank};
use crate::geometry::{DU, node_width, pin_color};
use crate::i18n::{self, t};
use crate::state::{PaletteDrag, use_editor};
use crate::ui::*;

mod drag;
pub use drag::*;

/// Pointer movement threshold (px) before clicking a palette item becomes a drag.
pub const PALETTE_DRAG_THRESHOLD: f64 = 4.0;
const LEFT_TAB_KEY: &str = "ticknode.left-tab";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftTab {
    #[default]
    Nodes,
    Resources,
    Variables,
}

fn load_left_tab() -> LeftTab {
    match window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(LEFT_TAB_KEY).ok().flatten())
        .as_deref()
    {
        Some("resources") => LeftTab::Resources,
        Some("variables") => LeftTab::Variables,
        _ => LeftTab::Nodes,
    }
}

fn save_left_tab(tab: LeftTab) {
    if let Some(storage) = window().local_storage().ok().flatten() {
        let val = match tab {
            LeftTab::Nodes => "nodes",
            LeftTab::Resources => "resources",
            LeftTab::Variables => "variables",
        };
        let _ = storage.set_item(LEFT_TAB_KEY, val);
    }
}

// --- Left Panel ---

#[component]
pub fn Palette() -> impl IntoView {
    let ed = use_editor();
    let active_tab = RwSignal::new(load_left_tab());

    let set_tab = move |tab: LeftTab| {
        active_tab.set(tab);
        save_left_tab(tab);
    };

    view! {
        <aside
            class="panel left palette"
            class:hidden=move || !ed.show_palette.get()
            class:dragging-out=move || ed.palette_drag.with(|d| d.as_ref().is_some_and(|d| d.active()))
        >
            <nav class="left-tabs">
                <button
                    class="left-tab ctl"
                    class:st-selected=move || active_tab.get() == LeftTab::Nodes
                    title=move || t!("left-tab-nodes")
                    on:click=move |_| set_tab(LeftTab::Nodes)
                    on:pointerdown=|ev| ev.stop_propagation()
                >
                    <span class="mdi mdi-shape-outline" aria-hidden="true"></span>
                    <span>{move || t!("left-tab-nodes")}</span>
                </button>
                <button
                    class="left-tab ctl"
                    class:st-selected=move || active_tab.get() == LeftTab::Resources
                    title=move || t!("left-tab-resources")
                    on:click=move |_| set_tab(LeftTab::Resources)
                    on:pointerdown=|ev| ev.stop_propagation()
                >
                    <span class="mdi mdi-folder-search-outline" aria-hidden="true"></span>
                    <span>{move || t!("left-tab-resources")}</span>
                </button>
                <button
                    class="left-tab ctl"
                    class:st-selected=move || active_tab.get() == LeftTab::Variables
                    title=move || t!("left-tab-variables")
                    on:click=move |_| set_tab(LeftTab::Variables)
                    on:pointerdown=|ev| ev.stop_propagation()
                >
                    <span class="mdi mdi-variable" aria-hidden="true"></span>
                    <span>{move || t!("left-tab-variables")}</span>
                </button>
            </nav>
            <div class="left-content">
                {move || match active_tab.get() {
                    LeftTab::Nodes => view! { <NodesPanel /> }.into_any(),
                    LeftTab::Resources => view! { <ResourcesPanel /> }.into_any(),
                    LeftTab::Variables => view! { <VariablesPanel /> }.into_any(),
                }}
            </div>
        </aside>
    }
}

#[component]
fn NodesPanel() -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let groups = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        let mut groups: BTreeMap<(usize, String), Vec<NodeDefinition>> = BTreeMap::new();
        for def in ed.definitions.get() {
            let haystack = format!(
                "{} {} {} {}",
                i18n::node_title(&def),
                def.type_name,
                i18n::node_description(&def),
                category_label(&def.category),
            )
            .to_lowercase();
            if q.is_empty() || haystack.contains(&q) {
                groups
                    .entry((category_rank(&def.category), def.category.clone()))
                    .or_default()
                    .push(def);
            }
        }
        groups
            .into_iter()
            .map(|((_, category), defs)| (category, defs))
            .collect::<Vec<_>>()
    });
    // Category currently scrolled into view (highlighted in the left rail)
    let active = RwSignal::new(None::<String>);
    let list = NodeRef::<html::Div>::new();

    let update_active = move || {
        let Some(list) = list.get_untracked() else {
            return;
        };
        let top = list.scroll_top() as f64 + 2.0 * DU;
        let sections = list.query_selector_all("[data-category]").ok();
        let mut current = None;
        for i in 0..sections.as_ref().map_or(0, |s| s.length()) {
            let Some(el) = sections
                .as_ref()
                .and_then(|s| s.item(i))
                .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
            else {
                continue;
            };
            if (el.offset_top() as f64) <= top || current.is_none() {
                current = el.get_attribute("data-category");
            }
        }
        active.set(current);
    };
    Effect::new(move |_| {
        groups.track();
        request_animation_frame(update_active);
    });
    let scroll_to = move |category: String| {
        let Some(list) = list.get_untracked() else {
            return;
        };
        let section = list
            .query_selector_all("[data-category]")
            .ok()
            .and_then(|s| {
                (0..s.length())
                    .filter_map(|i| s.item(i)?.dyn_into::<web_sys::HtmlElement>().ok())
                    .find(|el| {
                        el.get_attribute("data-category").as_deref() == Some(category.as_str())
                    })
            });
        if let Some(section) = section {
            list.set_scroll_top(section.offset_top());
            active.set(Some(category));
        }
    };

    let add = move |type_name: String| {
        let (cx, cy) = view_center(ed);
        // Small offset so multiple newly added nodes don't stack directly on top of each other
        let shift = (ed.nodes.with_untracked(|n| n.len()) % 6) as f64 * 4.0 * DU;
        let position = Position {
            x: (cx - node_width(&type_name) / 2.0 + shift) as f32,
            y: (cy - 10.0 * DU + shift) as f32,
        };
        ed.send(ClientMessage::AddNode {
            type_name,
            position,
        });
        if ed.narrow.get_untracked() {
            ed.show_palette.set(false);
        }
    };

    view! {
        <div class="palette-nodes">
            <nav class="palette-rail">
                {move || groups.get().into_iter().map(|(category, _)| {
                    let key = category.clone();
                    let is_active = move || active.with(|a| a.as_deref() == Some(key.as_str()));
                    let target = category.clone();
                    let label_cat = category.clone();
                    view! {
                        <button
                            class="rail-item"
                            class:active=is_active
                            style=format!("--cat-color:{}", category_color(&category))
                            title=category.clone()
                            on:click=move |_| scroll_to(target.clone())
                        >
                            <span class="rail-swatch"><Icon path=category_icon(&category) /></span>
                            <span class="rail-label">{move || category_label(&label_cat)}</span>
                        </button>
                    }
                }).collect_view()}
            </nav>
            <div class="palette-main">
                <div class="panel-section palette-search">
                    <TextField
                        value=Signal::derive(String::new)
                        placeholder=Signal::derive(move || t!("palette-search-placeholder"))
                        icon=icons::SEARCH
                        on_input=Callback::new(move |v| query.set(v))
                    />
                </div>
                <div class="palette-list" node_ref=list on:scroll=move |_| update_active()>
                    {move || {
                        let groups = groups.get();
                        if groups.is_empty() {
                            return view! { <div class="empty">{move || t!("palette-empty")}</div> }.into_any();
                        }
                        groups.into_iter().map(|(category, defs)| {
                            let color = category_color(&category);
                            let group_cat = category.clone();
                            view! {
                                <section class="palette-group" data-category=category.clone() style=format!("--cat-color:{color}")>
                                    <div class="palette-group-title">
                                        <span class="group-icon"><Icon path=category_icon(&category) /></span>
                                        {move || category_label(&group_cat)}
                                    </div>
                                    {defs.into_iter().map(|def| view! { <PaletteBlock def=def color=color add=add /> }).collect_view()}
                                </section>
                            }
                        }).collect_view().into_any()
                    }}
                </div>
            </div>
        </div>
    }
}

/// Clicking adds the node to the center; dragging drops it at the release position.
#[component]
fn PaletteBlock(
    def: NodeDefinition,
    color: &'static str,
    add: impl Fn(String) + Copy + 'static,
) -> impl IntoView {
    let ed = use_editor();
    let def_drag = def.clone();
    let start_drag = move |ev: PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        let at = (ev.client_x() as f64, ev.client_y() as f64);
        ed.palette_drag.set(Some(PaletteDrag::Node {
            type_name: def_drag.type_name.clone(),
            title: i18n::node_title(&def_drag),
            color,
            start: at,
            at,
            active: false,
        }));
    };
    let pin_chips = |dir: PinDirection| {
        let node_type = def.type_name.clone();
        def.pins.iter().filter(|p| p.direction == dir).map(|p| {
            let exec = p.pin_type == PinType::Exec;
            let node_type_title = node_type.clone();
            let pin_name_title = p.name.clone();
            let pin_type_title = p.pin_type.clone();
            let chip_title = move || {
                let pin_label = i18n::node_pin_label(&node_type_title, &pin_name_title);
                let type_name = match &pin_type_title {
                    PinType::Exec => i18n::type_name_label("exec"),
                    PinType::Data(t) => ed.type_title(t),
                };
                let technical = match &pin_type_title {
                    PinType::Data(t) => format!("\n{}", t!("pin-technical-type", name = data_type_name(t))),
                    _ => String::new(),
                };
                format!("{pin_label}: {type_name}{technical}")
            };
            let node_type_label = node_type.clone();
            let pin_name_label = p.name.clone();
            view! {
                <span class="block-pin" title=chip_title>
                    <span class="socket connected" class:exec=exec style=format!("--pin-color:{}", ed.types.with_untracked(|types| pin_color(types, &p.pin_type)))></span>
                    {(!exec).then(move || i18n::node_pin_label(&node_type_label, &pin_name_label))}
                </span>
            }
        }).collect_view()
    };
    let in_chips = pin_chips(PinDirection::Input);
    let out_chips = pin_chips(PinDirection::Output);
    let def_title = def.clone();
    let def_desc = def.clone();
    view! {
        <div
            class="palette-block palette-item"
            on:pointerdown=start_drag
            on:pointermove=move |ev: PointerEvent| move_palette_drag(ed, &ev)
            on:pointerup=move |ev: PointerEvent| match ed.palette_drag.try_update(Option::take).flatten() {
                Some(drag) if drag.active() => drop_on_canvas(ed, &drag, &ev),
                // Click without dragging — add to center
                Some(PaletteDrag::Node { type_name, .. }) => add(type_name),
                _ => {}
            }
            on:pointercancel=move |_| ed.palette_drag.set(None)
        >
            <div class="block-head">
                <span class="block-title ellipsis">{move || i18n::node_title(&def_title)}</span>
                <InfoButton def=def />
            </div>
            <div class="block-pins">
                <span class="block-pins-side in">{in_chips}</span>
                <span class="block-pins-side out">{out_chips}</span>
            </div>
            <div class="block-desc">{move || i18n::node_description(&def_desc)}</div>
        </div>
    }
}
