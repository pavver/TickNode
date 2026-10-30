//! Searchable picker for dynamic properties (`PropertyKind::Dynamic`): a field on the node that shows the
//! current value as icon + label, and a popup list with a search box, like Home Assistant's entity picker.
//! The popup is a fixed-position layer at the root of the app (not inside the node): nodes sit in a
//! transformed canvas, which would break fixed positioning and stack the popup under other nodes.

use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;

use crate::geometry::DU;
use crate::i18n::t;
use crate::state::{Editor, PickerState, use_editor};

/// How many matching rows are rendered at most (the rest is reached by refining the search).
pub const MAX_ROWS: usize = 100;
const MARGIN: f64 = 2.0 * DU;
const MIN_WIDTH: f64 = 80.0 * DU;
const MAX_HEIGHT: f64 = 90.0 * DU;
/// Below this much free space under the field the popup opens upwards (if there is more room above).
const MIN_SPACE_BELOW: f64 = 55.0 * DU;

/// CSS classes of a Material Design Icons glyph: `mdi:lightbulb` -> `mdi mdi-lightbulb`. Other icon sets
/// and malformed names have no glyph in the bundled font.
pub fn mdi_class(icon: &str) -> Option<String> {
    let name = icon.strip_prefix("mdi:")?;
    let valid = !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    valid.then(|| format!("mdi mdi-{name}"))
}

/// Indices of the options matching `query` (case-insensitive, every whitespace-separated term must occur in the
/// label, the detail or the value), at most `limit` of them, and how many matched in total.
pub fn filter_options(options: &[PropertyOption], query: &str, limit: usize) -> (Vec<usize>, usize) {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut shown = Vec::new();
    let mut total = 0;
    for (i, option) in options.iter().enumerate() {
        let matches = terms.is_empty() || {
            let haystack = format!("{}\n{}\n{}", option.label, option.detail, option.value).to_lowercase();
            terms.iter().all(|term| haystack.contains(term.as_str()))
        };
        if matches {
            total += 1;
            if shown.len() < limit {
                shown.push(i);
            }
        }
    }
    (shown, total)
}

/// Where the popup goes, in client coordinates: `left`, `width`, the vertical anchor and the maximum height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub left: f64,
    pub width: f64,
    /// `true`: `bottom` is the distance from the viewport bottom (opens upwards); otherwise `top`.
    pub up: bool,
    pub edge: f64,
    pub max_height: f64,
}

/// `anchor` is (left, top, right, bottom) of the field, `viewport` the window size.
pub fn placement(anchor: (f64, f64, f64, f64), viewport: (f64, f64)) -> Placement {
    let (left, top, right, bottom) = anchor;
    let (vw, vh) = viewport;
    let width = (right - left).max(MIN_WIDTH).min((vw - 2.0 * MARGIN).max(0.0));
    let x = left.clamp(MARGIN, (vw - width - MARGIN).max(MARGIN));
    let below = vh - bottom - MARGIN;
    let above = top - MARGIN;
    let up = below < MIN_SPACE_BELOW && above > below;
    let space = if up { above } else { below };
    Placement {
        left: x,
        width,
        up,
        edge: if up { vh - top } else { bottom },
        max_height: space.min(MAX_HEIGHT).max(10.0 * DU),
    }
}

fn viewport() -> (f64, f64) {
    (
        window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0),
        window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0),
    )
}

fn options_of(ed: Editor, plugin: &str, source: &str) -> std::sync::Arc<Vec<PropertyOption>> {
    ed.property_options.with(|all| all.get(&(plugin.to_string(), source.to_string())).cloned()).unwrap_or_default()
}

fn glyph(icon: Option<&str>) -> impl IntoView + use<> {
    let class = icon.and_then(mdi_class).unwrap_or_default();
    view! { <span class=format!("picker-icon {class}") aria-hidden="true"></span> }
}

/// The picker field on a node: the current value (icon + label) as a button that opens the popup.
#[component]
pub fn PickerField(
    node: NodeId,
    property: String,
    plugin: String,
    source: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let (p, s) = (plugin.clone(), source.clone());
    // The option behind the current value (if the list has it), otherwise the raw value is shown
    let current = Memo::new(move |_| {
        let value = value.get();
        let options = ed.property_options.with(|all| all.get(&(p.clone(), s.clone())).cloned()).unwrap_or_default();
        let found = options.iter().find(|o| o.value == value).cloned();
        (value, found)
    });
    let (prop, plug, src) = (property.clone(), plugin, source);
    let toggle = move |ev: web_sys::MouseEvent| {
        let open_here = ed.picker.with_untracked(|p| p.as_ref().is_some_and(|p| p.node == node && p.property == prop));
        if open_here {
            ed.picker.set(None);
            return;
        }
        let Some(rect) = ev.current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|el| el.get_bounding_client_rect())
        else { return };
        ed.picker.set(Some(PickerState {
            node,
            property: prop.clone(),
            plugin: plug.clone(),
            source: src.clone(),
            anchor: (rect.left(), rect.top(), rect.right(), rect.bottom()),
        }));
    };
    view! {
        <button
            class="inline-value picker-field"
            title=move || {
                let (value, found) = current.get();
                match found {
                    Some(o) if !o.detail.is_empty() => format!("{} - {}", title.get(), o.detail),
                    _ if !value.is_empty() => format!("{} - {value}", title.get()),
                    _ => title.get(),
                }
            }
            on:pointerdown=|ev| ev.stop_propagation()
            on:click=toggle
        >
            {move || {
                let (value, found) = current.get();
                if value.is_empty() {
                    view! { <span class="picker-label picker-placeholder">{t!("picker-placeholder")}</span> }.into_any()
                } else {
                    match found {
                        Some(o) => view! {
                            {glyph(o.icon.as_deref())}
                            <span class="picker-label">{o.label}</span>
                        }.into_any(),
                        None => view! { <span class="picker-label">{value}</span> }.into_any(),
                    }
                }
            }}
        </button>
    }
}

/// Mounts the popup of the open picker; the app renders it once, at its root.
#[component]
pub fn PickerLayer() -> impl IntoView {
    let ed = use_editor();
    // Panning or zooming moves the field away from the popup: close it
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() {
            ed.picker.set(None);
        }
    });
    move || ed.picker.get().map(|state| view! { <PickerPopup state=state /> })
}

#[component]
fn PickerPopup(state: PickerState) -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(el) = input.get() {
            let _ = el.focus();
        }
    });
    let (node, property) = (state.node, state.property.clone());
    // The node may vanish (removed, plugin disabled) while the popup is open
    Effect::new(move |_| {
        if ed.node_tracked(node).is_none() {
            ed.picker.set(None);
        }
    });
    let current = {
        let property = property.clone();
        Memo::new(move |_| {
            ed.node_tracked(node)
                .and_then(|n| n.instance.with(|i| i.model.properties.iter().find(|(name, _)| *name == property).map(|(_, v)| v.clone())))
                .and_then(|v| match v {
                    DataValue::String(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_default()
        })
    };
    let (plugin, source) = (state.plugin.clone(), state.source.clone());
    let options = Memo::new(move |_| options_of(ed, &plugin, &source));
    let found = Memo::new(move |_| {
        let options = options.get();
        query.with(|q| filter_options(&options, q, MAX_ROWS))
    });

    let pick = {
        let property = property.clone();
        move |value: String| {
            if value != current.get_untracked() {
                ed.send(ClientMessage::SetProperty { id: node, name: property.clone(), value: DataValue::String(value) });
            }
            ed.picker.set(None);
        }
    };
    let scroll_to = move |i: usize| {
        if let Some(el) = document().get_element_by_id(&format!("picker-opt-{i}")) {
            let opts = web_sys::ScrollIntoViewOptions::new();
            opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
            el.scroll_into_view_with_scroll_into_view_options(&opts);
        }
    };
    let on_key = {
        let pick = pick.clone();
        move |ev: web_sys::KeyboardEvent| {
            let count = found.with_untracked(|(shown, _)| shown.len());
            match ev.key().as_str() {
                "ArrowDown" => {
                    ev.prevent_default();
                    if count > 0 {
                        let next = (active.get_untracked() + 1).min(count - 1);
                        active.set(next);
                        scroll_to(next);
                    }
                }
                "ArrowUp" => {
                    ev.prevent_default();
                    let next = active.get_untracked().saturating_sub(1);
                    active.set(next);
                    scroll_to(next);
                }
                "Enter" => {
                    ev.prevent_default();
                    let chosen = found.with_untracked(|(shown, _)| shown.get(active.get_untracked()).copied());
                    let typed = query.get_untracked().trim().to_string();
                    match chosen {
                        Some(i) => pick(options.get_untracked()[i].value.clone()),
                        // No match: the typed text itself is the value (an entity the list does not know yet)
                        None if !typed.is_empty() => pick(typed),
                        None => {}
                    }
                }
                "Escape" => {
                    ev.prevent_default();
                    ed.picker.set(None);
                }
                _ => {}
            }
        }
    };

    let place = placement(state.anchor, viewport());
    let vertical = if place.up { format!("bottom:{}px", place.edge) } else { format!("top:{}px", place.edge) };
    let style = format!("left:{}px;width:{}px;{vertical};max-height:{}px", place.left, place.width, place.max_height);
    let row_pick = pick.clone();
    let typed_pick = pick;
    view! {
        <div class="picker-popup" style=style on:pointerdown=|ev| ev.stop_propagation()>
            <input
                node_ref=input
                class="picker-search"
                type="text"
                spellcheck="false"
                autocomplete="off"
                placeholder=move || t!("picker-search-placeholder")
                prop:value=move || query.get()
                on:input=move |ev| {
                    query.set(event_target_value(&ev));
                    active.set(0);
                }
                on:keydown=on_key
            />
            <div class="picker-list">
                {move || {
                    let options = options.get();
                    let (shown, _) = found.get();
                    let selected = current.get();
                    let row_pick = row_pick.clone();
                    shown.into_iter().enumerate().map(|(row, i)| {
                        let option = options[i].clone();
                        let value = option.value.clone();
                        let pick = row_pick.clone();
                        let is_current = option_is_current(&selected, &value);
                        let class = move || {
                            let mut class = String::from("picker-row");
                            if active.get() == row { class.push_str(" active"); }
                            if is_current { class.push_str(" current"); }
                            class
                        };
                        let value = option.value.clone();
                        view! {
                            <div
                                id=format!("picker-opt-{row}")
                                class=class
                                on:mousemove=move |_| if active.get_untracked() != row { active.set(row) }
                                on:click=move |_| pick(value.clone())
                            >
                                {glyph(option.icon.as_deref())}
                                <span class="picker-text">
                                    <span class="picker-row-label">{option.label.clone()}</span>
                                    {(!option.detail.is_empty() && option.detail != option.label).then(|| view! { <span class="picker-row-detail">{option.detail.clone()}</span> })}
                                </span>
                            </div>
                        }
                    }).collect_view()
                }}
                {move || {
                    let (shown, total) = found.get();
                    let typed = query.get().trim().to_string();
                    let exact = options.with(|o| o.iter().any(|o| o.value == typed));
                    let note = if options.with(|o| o.is_empty()) && typed.is_empty() {
                        Some(t!("picker-no-options"))
                    } else if shown.is_empty() && typed.is_empty() {
                        Some(t!("picker-empty"))
                    } else if total > shown.len() {
                        Some(t!("picker-more", count = total - shown.len()))
                    } else {
                        None
                    };
                    let use_typed = (!typed.is_empty() && !exact).then(|| {
                        let pick = typed_pick.clone();
                        let value = typed.clone();
                        view! {
                            <div class=move || if found.with(|(s, _)| s.is_empty()) { "picker-row picker-typed active" } else { "picker-row picker-typed" } on:click=move |_| pick(value.clone())>
                                <span class="picker-icon mdi mdi-keyboard-outline" aria-hidden="true"></span>
                                <span class="picker-text">
                                    <span class="picker-row-label">{t!("picker-use-typed", value = typed.clone())}</span>
                                </span>
                            </div>
                        }
                    });
                    view! {
                        {note.map(|n| view! { <div class="picker-note">{n}</div> })}
                        {use_typed}
                    }
                }}
            </div>
        </div>
    }
}

fn option_is_current(current: &str, value: &str) -> bool {
    !current.is_empty() && current == value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(value: &str, label: &str) -> PropertyOption {
        PropertyOption { value: value.into(), label: label.into(), detail: value.into(), icon: None }
    }

    #[test]
    fn mdi_icons_become_font_classes() {
        assert_eq!(mdi_class("mdi:lightbulb").as_deref(), Some("mdi mdi-lightbulb"));
        assert_eq!(mdi_class("mdi:door-open").as_deref(), Some("mdi mdi-door-open"));
        assert_eq!(mdi_class("mdi:battery-10").as_deref(), Some("mdi mdi-battery-10"));
        // Other icon sets and unsafe names have no glyph (and never reach the class attribute)
        assert_eq!(mdi_class("phu:bulb"), None);
        assert_eq!(mdi_class("mdi:"), None);
        assert_eq!(mdi_class("mdi:a b\" onclick=x"), None);
        assert_eq!(mdi_class("lightbulb"), None);
    }

    #[test]
    fn search_matches_label_detail_and_value_case_insensitively() {
        let options = vec![
            option("light.kitchen", "Kitchen Ceiling"),
            option("light.hall", "Hall"),
            option("sensor.kitchen_temp", "Temperature"),
        ];
        assert_eq!(filter_options(&options, "", 10), (vec![0, 1, 2], 3));
        assert_eq!(filter_options(&options, "KITCHEN", 10), (vec![0, 2], 2));
        // Every term must match somewhere (label or id)
        assert_eq!(filter_options(&options, "kitchen ceil", 10), (vec![0], 1));
        assert_eq!(filter_options(&options, "sensor temp", 10), (vec![2], 1));
        assert_eq!(filter_options(&options, "nothing", 10), (vec![], 0));
    }

    #[test]
    fn search_caps_the_rendered_rows_but_counts_all_matches() {
        let options: Vec<PropertyOption> = (0..2500).map(|i| option(&format!("sensor.s{i}"), &format!("Sensor {i}"))).collect();
        let (shown, total) = filter_options(&options, "", MAX_ROWS);
        assert_eq!((shown.len(), total), (MAX_ROWS, 2500));
        let (shown, total) = filter_options(&options, "sensor 12", MAX_ROWS);
        assert!(shown.len() <= MAX_ROWS && total >= shown.len());
        assert!(shown.iter().all(|i| options[*i].label.contains("12")));
    }

    #[test]
    fn popup_opens_below_or_above_and_stays_on_screen() {
        // Plenty of room below
        let p = placement((100.0, 100.0, 220.0, 124.0), (1280.0, 800.0));
        assert!(!p.up && p.edge == 124.0 && p.left == 100.0);
        assert_eq!(p.width, MIN_WIDTH);
        // Near the bottom: opens upwards, anchored to the field's top edge
        let p = placement((100.0, 760.0, 220.0, 784.0), (1280.0, 800.0));
        assert!(p.up && p.edge == 40.0);
        // Narrow screen: never wider than the viewport and clamped to its edges
        let p = placement((250.0, 100.0, 330.0, 124.0), (360.0, 640.0));
        assert!(p.width <= 360.0 - 2.0 * MARGIN && p.left + p.width <= 360.0 - MARGIN + 0.001 && p.left >= MARGIN);
        // A wide field gets a popup as wide as itself
        let p = placement((50.0, 100.0, 450.0, 124.0), (1280.0, 800.0));
        assert_eq!(p.width, 400.0);
    }
}
