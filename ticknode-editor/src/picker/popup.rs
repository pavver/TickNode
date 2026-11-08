//! Searchable popup list and keyboard navigation.

use super::*;

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

pub(super) fn option_is_current(current: &str, value: &str) -> bool {
    !current.is_empty() && current == value
}
