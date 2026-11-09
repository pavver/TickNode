//! Shared searchable node popup and dismissal behavior for canvas menus.

use leptos::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};

use crate::categories::{category_color, category_icon};
use crate::i18n::t;
use crate::picker::{KeyAction, key_action, placement, viewport};
use crate::state::use_editor;
use crate::ui::Icon;

#[derive(Clone, PartialEq)]
pub struct MenuRow {
    pub index: usize,
    pub title: String,
    pub detail: String,
    pub category: Option<String>,
    /// Section heading; a divider is drawn wherever it changes between consecutive rows.
    pub section: Option<String>,
}

#[derive(Clone, Default, PartialEq)]
pub struct MenuRows {
    pub shown: Vec<MenuRow>,
    pub total: usize,
}

/// Capture sees presses even on controls that stop propagation at the node.
pub fn dismiss_menu(selector: &'static str, close: Callback<()>) {
    let ed = use_editor();
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() { close.run(()); }
    });
    let outside = Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |ev: web_sys::PointerEvent| {
        let inside = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(selector).ok().flatten()).is_some();
        if !inside { close.run(()); }
    });
    let _ = window().add_event_listener_with_callback_and_bool("pointerdown", outside.as_ref().unchecked_ref(), true);
    let outside = StoredValue::new_local(outside);
    on_cleanup(move || outside.with_value(|listener| {
        let _ = window().remove_event_listener_with_callback_and_bool("pointerdown", listener.as_ref().unchecked_ref(), true);
    }));
}

#[component]
pub fn NodeMenuPopup(
    screen: (f64, f64),
    class: &'static str,
    id_prefix: &'static str,
    rows: Callback<String, MenuRows>,
    pick: Callback<usize>,
    close: Callback<()>,
    empty_text: Signal<String>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(el) = input.get() { let _ = el.focus(); }
    });
    let found = Memo::new(move |_| rows.run(query.get()));
    let on_key = move |ev: web_sys::KeyboardEvent| {
        let count = found.with_untracked(|r| r.shown.len());
        let action = key_action(&ev.key(), active.get_untracked(), count);
        if action != KeyAction::Ignore { ev.prevent_default(); ev.stop_propagation(); }
        match action {
            KeyAction::Move(next) => {
                active.set(next);
                if let Some(el) = document().get_element_by_id(&format!("{id_prefix}-{next}")) {
                    let options = web_sys::ScrollIntoViewOptions::new();
                    options.set_block(web_sys::ScrollLogicalPosition::Nearest);
                    el.scroll_into_view_with_scroll_into_view_options(&options);
                }
            }
            KeyAction::Pick => {
                if let Some(index) = found.with_untracked(|r| r.shown.get(active.get_untracked()).map(|r| r.index)) { pick.run(index); }
            }
            KeyAction::Close => close.run(()),
            KeyAction::Handled | KeyAction::Ignore => {}
        }
    };
    let (x, y) = screen;
    let place = placement((x, y, x, y), viewport());
    let vertical = if place.up { format!("bottom:{}px", place.edge) } else { format!("top:{}px", place.edge) };
    let style = format!("left:{}px;width:{}px;{vertical};max-height:{}px", place.left, place.width, place.max_height);
    view! {
        <div class=class style=style on:pointerdown=|ev| ev.stop_propagation()>
            <input node_ref=input class="picker-search" type="text" spellcheck="false" autocomplete="off"
                placeholder=move || t!("picker-search-placeholder")
                prop:value=move || query.get()
                on:input=move |ev| { query.set(event_target_value(&ev)); active.set(0); }
                on:keydown=on_key
            />
            <div class="picker-list">
                {move || {
                    let result = found.get();
                    let mut previous: Option<String> = None;
                    result.shown.into_iter().enumerate().map(|(row, choice)| {
                        let divider = choice.section.clone().filter(|s| previous.as_ref() != Some(s));
                        previous = choice.section.clone();
                        view! {
                            {divider.map(|label| view! { <div class="picker-note node-menu-divider">{label}</div> })}
                            <div id=format!("{id_prefix}-{row}")
                                class=move || if active.get() == row { "picker-row active" } else { "picker-row" }
                                on:mousemove=move |_| if active.get_untracked() != row { active.set(row); }
                                on:click=move |_| pick.run(choice.index)>
                                {choice.category.map(|category| view! {
                                    <span class="node-menu-icon" style=format!("color:{}", category_color(&category))>
                                        <Icon path=category_icon(&category) />
                                    </span>
                                })}
                                <span class="picker-text">
                                    <span class="picker-row-label">{choice.title}</span>
                                    <span class="picker-row-detail">{choice.detail}</span>
                                </span>
                            </div>
                        }
                    }).collect_view()
                }}
                {move || {
                    let result = found.get();
                    if result.shown.is_empty() {
                        Some(view! { <div class="picker-note">{empty_text.get()}</div> })
                    } else if result.total > result.shown.len() {
                        Some(view! { <div class="picker-note">{t!("picker-more", count = result.total - result.shown.len())}</div> })
                    } else { None }
                }}
            </div>
        </div>
    }
}
