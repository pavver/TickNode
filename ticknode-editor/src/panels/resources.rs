//! Resources tab in the left panel: dynamic options grouped by plugin collection,
//! draggable onto the canvas or clickable to create configured nodes.

use leptos::prelude::*;
use std::collections::HashSet;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use super::palette::{drop_on_canvas, handle_resource_click, move_palette_drag};
use crate::categories::category_color;
use crate::i18n::{self, t};
use crate::picker::{filter_options, mdi_class};
use crate::state::{PaletteDrag, PropertyOptions, use_editor};
use crate::ui::{TextField, icons};

const RESOURCE_LIMIT: usize = 100;
const COLLAPSED_KEY: &str = "ticknode.resources-collapsed";

fn load_collapsed() -> HashSet<String> {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(COLLAPSED_KEY).ok().flatten())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_collapsed(collapsed: &HashSet<String>) {
    if let (Some(storage), Ok(json)) = (
        window().local_storage().ok().flatten(),
        serde_json::to_string(collapsed),
    ) {
        let _ = storage.set_item(COLLAPSED_KEY, &json);
    }
}

/// Actions for an item: the collection's `actions`, filtered by `requires_source`:
/// keep an action only if the item's `value` is among the values of `Editor::property_options[(plugin, requires_source)]`.
pub fn available_actions<'a>(
    actions: &'a [ResourceAction],
    plugin: &str,
    item_value: &str,
    property_options: &PropertyOptions,
) -> Vec<&'a ResourceAction> {
    actions
        .iter()
        .filter(|action| match &action.requires_source {
            None => true,
            Some(source) => property_options
                .get(&(plugin.to_string(), source.clone()))
                .map_or(false, |options| {
                    options.iter().any(|opt| opt.value == item_value)
                }),
        })
        .collect()
}

#[component]
pub fn ResourcesPanel() -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let collapsed = RwSignal::new(load_collapsed());

    let toggle_collapse = move |key: String| {
        collapsed.update(|set| {
            if set.contains(&key) {
                set.remove(&key);
            } else {
                set.insert(key);
            }
            save_collapsed(set);
        });
    };

    let collections = Memo::new(move |_| {
        ed.plugins.with(|plugins| {
            plugins
                .iter()
                .filter(|p| p.state == PluginState::Enabled)
                .flat_map(|p| p.resources.iter().map(move |c| (p.name.clone(), c.clone())))
                .collect::<Vec<_>>()
        })
    });

    view! {
        <div class="palette-main">
            <div class="panel-section palette-search">
                <TextField
                    value=Signal::derive(String::new)
                    placeholder=Signal::derive(move || t!("picker-search-placeholder"))
                    icon=icons::SEARCH
                    on_input=Callback::new(move |v| query.set(v))
                />
            </div>
            <div class="palette-list">
                {move || {
                    let all = collections.get();
                    if all.is_empty() {
                        return view! { <div class="empty">{move || t!("resources-empty")}</div> }.into_any();
                    }
                    let q = query.get();
                    all.into_iter().map(|(plugin, col)| {
                        view! {
                            <CollectionSection
                                plugin=plugin
                                col=col
                                query=q.clone()
                                collapsed=collapsed
                                on_toggle=toggle_collapse
                            />
                        }
                    }).collect_view().into_any()
                }}
            </div>
        </div>
    }
}

#[component]
fn CollectionSection(
    plugin: String,
    col: ResourceCollection,
    query: String,
    collapsed: RwSignal<HashSet<String>>,
    on_toggle: impl Fn(String) + Copy + 'static,
) -> impl IntoView {
    let ed = use_editor();
    let col_key = format!("{}:{}", plugin, col.id);
    let key_for_toggle = col_key.clone();
    let is_collapsed = Memo::new(move |_| collapsed.with(|s| s.contains(&col_key)));

    let items = Memo::new({
        let plugin = plugin.clone();
        let source = col.source.clone();
        move |_| {
            ed.property_options.with(|opts| {
                opts.get(&(plugin.clone(), source.clone()))
                    .cloned()
                    .unwrap_or_default()
            })
        }
    });

    let filtered = Memo::new({
        let q = query.clone();
        move |_| {
            let opts = items.get();
            filter_options(&opts, &q, RESOURCE_LIMIT)
        }
    });

    let col_icon = col.icon.clone();
    let col_title_def = col.clone();
    let p_name = plugin.clone();
    let title_fn = move || i18n::resource_collection_title(&p_name, &col_title_def);

    view! {
        <section class="palette-group">
            <div
                class="palette-group-title resource-group-title"
                on:click=move |_| on_toggle(key_for_toggle.clone())
                on:pointerdown=|ev| ev.stop_propagation()
            >
                <span class=move || if is_collapsed.get() {
                    "mdi mdi-chevron-right resource-group-chevron"
                } else {
                    "mdi mdi-chevron-down resource-group-chevron"
                } aria-hidden="true"></span>
                {mdi_class(&col_icon).map(|cls| view! {
                    <span class=format!("resource-group-icon {cls}") aria-hidden="true"></span>
                })}
                <span class="resource-group-label ellipsis">{title_fn}</span>
                <span class="resource-group-count">
                    {move || {
                        let opts = items.get();
                        if opts.is_empty() {
                            0
                        } else {
                            filtered.get().1
                        }
                    }}
                </span>
            </div>
            {move || {
                if is_collapsed.get() {
                    return None;
                }
                let opts = items.get();
                if opts.is_empty() {
                    return Some(view! { <div class="empty">{move || t!("resources-loading")}</div> }.into_any());
                }
                let (shown_indices, total) = filtered.get();
                if shown_indices.is_empty() {
                    return Some(view! { <div class="empty">{move || t!("palette-empty")}</div> }.into_any());
                }
                let plugin = plugin.clone();
                let col_id = col.id.clone();
                let col_icon = col.icon.clone();
                let col_actions = col.actions.clone();
                let rows_view = shown_indices.into_iter().map(move |idx| {
                    let item = opts[idx].clone();
                    view! {
                        <ResourceRow
                            plugin=plugin.clone()
                            collection_id=col_id.clone()
                            default_icon=col_icon.clone()
                            item=item
                            all_actions=col_actions.clone()
                        />
                    }
                }).collect_view();

                let remaining = total.saturating_sub(RESOURCE_LIMIT);
                Some(view! {
                    <div>
                        {rows_view}
                        {(remaining > 0).then(move || view! {
                            <div class="resource-more-note">
                                {move || t!("resources-more", count = remaining)}
                            </div>
                        })}
                    </div>
                }.into_any())
            }}
        </section>
    }
}

#[component]
fn ResourceRow(
    plugin: String,
    collection_id: String,
    default_icon: String,
    item: PropertyOption,
    all_actions: Vec<ResourceAction>,
) -> impl IntoView {
    let ed = use_editor();
    let item_for_drag = item.clone();
    let item_for_click = item.clone();
    let plugin_for_drag = plugin.clone();
    let col_id_for_drag = collection_id.clone();
    let plugin_for_click = plugin.clone();
    let col_id_for_click = collection_id.clone();
    let actions_for_drag = all_actions.clone();
    let actions_for_click = all_actions.clone();

    let icon_class = item
        .icon
        .as_deref()
        .and_then(mdi_class)
        .or_else(|| mdi_class(&default_icon));

    let detail_text = if !item.detail.is_empty() && item.detail != item.label {
        item.detail.clone()
    } else {
        item.value.clone()
    };

    let start_drag = move |ev: PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        ev.stop_propagation();
        let available: Vec<ResourceAction> = ed.property_options.with_untracked(|opts| {
            available_actions(
                &actions_for_drag,
                &plugin_for_drag,
                &item_for_drag.value,
                opts,
            )
            .into_iter()
            .cloned()
            .collect()
        });
        if available.is_empty() {
            return;
        }
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        let at = (ev.client_x() as f64, ev.client_y() as f64);
        let color = ed.definitions.with_untracked(|defs| {
            available
                .first()
                .and_then(|a| defs.iter().find(|d| d.type_name == a.type_name))
                .map(|d| category_color(&d.category))
                .unwrap_or("#00B7C2")
        });
        ed.palette_drag.set(Some(PaletteDrag::Resource {
            plugin: plugin_for_drag.clone(),
            collection_id: col_id_for_drag.clone(),
            item: item_for_drag.clone(),
            actions: available,
            title: item_for_drag.label.clone(),
            color,
            start: at,
            at,
            active: false,
        }));
    };

    let on_pointer_up = move |ev: PointerEvent| {
        match ed.palette_drag.try_update(Option::take).flatten() {
            Some(drag) if drag.active() => drop_on_canvas(ed, &drag, &ev),
            Some(PaletteDrag::Resource {
                plugin,
                collection_id,
                item,
                actions,
                ..
            }) => {
                let screen = (ev.client_x() as f64, ev.client_y() as f64);
                handle_resource_click(ed, &plugin, &collection_id, &item, &actions, screen);
            }
            Some(drag) => {
                // If not active and not Resource, do standard drop/click
                drop_on_canvas(ed, &drag, &ev);
            }
            None => {
                // Direct click without prior drag start
                let available: Vec<ResourceAction> = ed.property_options.with_untracked(|opts| {
                    available_actions(
                        &actions_for_click,
                        &plugin_for_click,
                        &item_for_click.value,
                        opts,
                    )
                    .into_iter()
                    .cloned()
                    .collect()
                });
                if !available.is_empty() {
                    let screen = (ev.client_x() as f64, ev.client_y() as f64);
                    handle_resource_click(
                        ed,
                        &plugin_for_click,
                        &col_id_for_click,
                        &item_for_click,
                        &available,
                        screen,
                    );
                }
            }
        }
    };

    view! {
        <div
            class="resource-row palette-item"
            on:pointerdown=start_drag
            on:pointermove=move |ev: PointerEvent| move_palette_drag(ed, &ev)
            on:pointerup=on_pointer_up
            on:pointercancel=move |_| ed.palette_drag.set(None)
        >
            <span class="resource-icon">
                {icon_class.map(|cls| view! { <span class=cls aria-hidden="true"></span> })}
            </span>
            <span class="resource-text">
                <span class="resource-label">{item.label}</span>
                <span class="resource-detail">{detail_text}</span>
            </span>
        </div>
    }
}

#[cfg(test)]
mod tests;
