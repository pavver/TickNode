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
    let on_pick = state.on_pick;
    let supplied_value = state.value;
    Effect::new(move |_| {
        if on_pick.is_none() && ed.node_tracked(node).is_none() {
            ed.picker.set(None);
        }
    });
    let current = {
        let property = property.clone();
        Memo::new(move |_| {
            if let Some(value) = supplied_value { return value.get(); }
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
    let input_property = property.clone();
    let options = Memo::new(move |_| {
        let options = options_of(ed, &plugin, &source);
        if on_pick.is_none() && property_has_input(ed, node, &input_property) {
            std::sync::Arc::new(input_options(&options, &t!("property-from-input")))
        } else { options }
    });
    let found = Memo::new(move |_| {
        let options = options.get();
        query.with(|q| filter_options(&options, q, MAX_ROWS))
    });

    let pick = {
        let property = property.clone();
        move |value: String| {
            if let Some(callback) = on_pick {
                callback.run(value);
            } else if value != current.get_untracked() {
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
            let action = key_action(&ev.key(), active.get_untracked(), count);
            if action != KeyAction::Ignore { ev.prevent_default(); }
            match action {
                KeyAction::Move(next) => { active.set(next); scroll_to(next); }
                KeyAction::Pick => {
                    let chosen = found.with_untracked(|(shown, _)| shown.get(active.get_untracked()).copied());
                    if let Some(value) = chosen_value(&options.get_untracked(), chosen, &query.get_untracked()) { pick(value); }
                }
                KeyAction::Close => ed.picker.set(None),
                KeyAction::Handled | KeyAction::Ignore => {}
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
                    let note = list_note(options.with(|o| o.is_empty()), typed.is_empty(), shown.len(), total);
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
    current == value
}

#[derive(Debug, PartialEq)]
pub(crate) enum KeyAction { Move(usize), Pick, Close, Handled, Ignore }

pub(crate) fn key_action(key: &str, active: usize, count: usize) -> KeyAction {
    match key {
        "ArrowDown" if count > 0 => KeyAction::Move((active + 1).min(count - 1)),
        "ArrowDown" => KeyAction::Handled,
        "ArrowUp" => KeyAction::Move(active.saturating_sub(1)),
        "Enter" => KeyAction::Pick,
        "Escape" => KeyAction::Close,
        _ => KeyAction::Ignore,
    }
}

fn chosen_value(options: &[PropertyOption], chosen: Option<usize>, typed: &str) -> Option<String> {
    match chosen {
        Some(i) => Some(options[i].value.clone()),
        None if !typed.trim().is_empty() => Some(typed.trim().to_string()),
        None => None,
    }
}

fn list_note(no_options: bool, no_query: bool, shown: usize, total: usize) -> Option<String> {
    if no_options && no_query { Some(t!("picker-no-options")) }
    else if shown == 0 && no_query { Some(t!("picker-empty")) }
    else if total > shown { Some(t!("picker-more", count = total - shown)) }
    else { None }
}

#[cfg(test)]
mod tests;
