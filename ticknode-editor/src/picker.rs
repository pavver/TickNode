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

mod popup;

pub use popup::*;

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

#[cfg(test)]
mod tests;
