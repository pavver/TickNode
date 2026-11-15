//! In-place mathematical formula property editing with completion, signature help, and preview.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::state::use_editor;

pub mod auto_close;
pub mod completion;
pub mod highlight;
pub mod hover;
pub mod preview;
pub mod signature_help;

#[cfg(test)]
mod tests;

pub use highlight::FormulaHighlightLayer;
pub use preview::FormulaStatusRow;

/// Reactive context storing the current draft of the formula being edited on a node.
#[derive(Clone, Copy)]
pub struct FormulaDraft(pub RwSignal<String>);

/// Textarea editor for the `expression` property of `math/expression` nodes.
#[component]
pub fn FormulaField(
    id: NodeId,
    #[prop(into)] value: Signal<Option<DataValue>>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] row_height: Signal<f64>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let _ = row_height;
    let ed = use_editor();
    let focused = RwSignal::new(false);
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let pre_ref = NodeRef::<leptos::html::Pre>::new();

    let sync_scroll = move || {
        #[cfg(target_arch = "wasm32")]
        if let (Some(ta), Some(pre)) = (textarea_ref.get(), pre_ref.get()) {
            pre.set_scroll_top(ta.scroll_top());
            pre.set_scroll_left(ta.scroll_left());
        }
    };

    let input_names = Signal::derive(move || {
        ed.node_tracked(id).map_or(Vec::new(), |node| {
            node.instance.with(|i| {
                i.definition
                    .pins
                    .iter()
                    .filter(|p| {
                        p.direction == PinDirection::Input && matches!(p.pin_type, PinType::Data(_))
                    })
                    .map(|p| p.name.clone())
                    .collect()
            })
        })
    });

    let initial_value = match value.get_untracked() {
        Some(DataValue::String(s)) => s,
        _ => String::new(),
    };
    let draft = use_context::<FormulaDraft>()
        .map(|d| d.0)
        .unwrap_or_else(|| RwSignal::new(initial_value));

    // When not focused, server updates overwrite draft
    Effect::new(move |_| {
        let stored = match value.get() {
            Some(DataValue::String(s)) => s,
            _ => String::new(),
        };
        if !focused.get_untracked() {
            draft.set(stored);
        }
    });

    let timer = StoredValue::<Option<leptos::prelude::TimeoutHandle>>::new(None);
    let last_sent = StoredValue::<Option<String>>::new(None);
    let last_pos = StoredValue::<Option<(f64, f64)>>::new(None);
    let last_time = StoredValue::<f64>::new(0.0);
    let last_hovered = StoredValue::<Option<hover::FormulaHoverToken>>::new(None);
    let last_auto_close = StoredValue::<Option<auto_close::AutoCloseState>>::new(None);

    on_cleanup(move || {
        if let Some(h) = timer.get_value() {
            h.clear();
        }
    });

    let trigger_completion = move |is_explicit: bool| {
        completion::request_completion(ed, id, is_explicit);
        crate::inline::formula_field::signature_help::update_signature_help(ed, id);
    };

    let keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        let is_open = ed
            .template_completion
            .with_untracked(|c| c.as_ref().is_some_and(|s| s.node == id));

        if (ev.ctrl_key() || ev.meta_key()) && ev.code() == "Space" {
            ev.prevent_default();
            ev.stop_propagation();
            trigger_completion(true);
            return;
        }

        if is_open {
            let count = ed
                .template_completion
                .with_untracked(|c| c.as_ref().map_or(0, |s| s.completion.items.len()));
            match key.as_str() {
                "ArrowDown" if count > 0 => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    ed.template_completion.update(|opt| {
                        if let Some(s) = opt {
                            s.selected = (s.selected + 1) % count;
                        }
                    });
                    return;
                }
                "ArrowUp" if count > 0 => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    ed.template_completion.update(|opt| {
                        if let Some(s) = opt {
                            s.selected = if s.selected == 0 {
                                count - 1
                            } else {
                                s.selected - 1
                            };
                        }
                    });
                    return;
                }
                "Tab" | "Enter" if count > 0 => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    let (item, range) = ed
                        .template_completion
                        .with_untracked(|opt| {
                            opt.as_ref().and_then(|s| {
                                s.completion
                                    .items
                                    .get(s.selected)
                                    .map(|it| (it.clone(), s.completion.range.clone()))
                            })
                        })
                        .unzip();
                    if let (Some(it), Some(rng)) = (item, range) {
                        crate::inline::template_field::completion::accept_completion_item(
                            ed, id, &it, rng,
                        );
                    }
                    return;
                }
                "Escape" => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    ed.template_completion.set(None);
                    ed.template_signature.set(None);
                    return;
                }
                _ => {}
            }
        } else if key == "Escape" && ed.template_signature.with_untracked(|s| s.is_some()) {
            ev.prevent_default();
            ev.stop_propagation();
            ed.template_signature.set(None);
            return;
        }

        // Auto-close handling for '('
        #[cfg(target_arch = "wasm32")]
        if let Some(textarea) = textarea_ref.get() {
            let current = textarea.value();
            let u16_start = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
            let u16_end = textarea.selection_end().ok().flatten().unwrap_or(0) as usize;
            let sel_start =
                crate::inline::template_field::caret::utf16_to_byte(&current, u16_start);
            let sel_end = crate::inline::template_field::caret::utf16_to_byte(&current, u16_end);

            let action = auto_close::decide_typing(
                &current,
                sel_start..sel_end,
                key.as_str(),
                last_auto_close.get_value(),
            );

            match action {
                auto_close::AutoCloseAction::None => {
                    last_auto_close.set_value(None);
                }
                auto_close::AutoCloseAction::Insert {
                    range,
                    text: ins_text,
                    new_caret,
                    state,
                } => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    use crate::inline::undoable;
                    let (u16_start, u16_end, u16_caret, _) =
                        undoable::plan_replace(&current, range, ins_text, new_caret);
                    let final_text =
                        undoable::replace_range(&textarea, u16_start, u16_end, ins_text, u16_caret);
                    draft.set(final_text);
                    sync_scroll();
                    last_auto_close.set_value(Some(state));
                }
                auto_close::AutoCloseAction::Wrap {
                    range,
                    new_text,
                    new_caret,
                } => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    use crate::inline::undoable;
                    let (u16_start, u16_end, u16_caret, _) =
                        undoable::plan_replace(&current, range, &new_text, new_caret);
                    let final_text = undoable::replace_range(
                        &textarea, u16_start, u16_end, &new_text, u16_caret,
                    );
                    draft.set(final_text);
                    sync_scroll();
                    last_auto_close.set_value(None);
                }
                auto_close::AutoCloseAction::SkipChar { new_caret } => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    let u16_caret =
                        crate::inline::template_field::caret::byte_to_utf16(&current, new_caret)
                            as u32;
                    let _ = textarea.set_selection_range(u16_caret, u16_caret);
                    last_auto_close.set_value(None);
                }
                auto_close::AutoCloseAction::DeletePair { range, new_caret } => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    use crate::inline::undoable;
                    let (u16_start, u16_end, u16_caret, _) =
                        undoable::plan_replace(&current, range, "", new_caret);
                    let final_text =
                        undoable::replace_range(&textarea, u16_start, u16_end, "", u16_caret);
                    draft.set(final_text);
                    sync_scroll();
                    last_auto_close.set_value(None);
                }
            }
        }
    };

    view! {
        <div class="formula-editor" on:pointerdown=|ev| ev.stop_propagation()>
            <FormulaHighlightLayer
                node_id=id
                pre_ref=pre_ref
                draft=Signal::derive(move || draft.get())
                inputs=input_names
            />
            <textarea
                id=format!("formula-textarea-{}", id.0)
                node_ref=textarea_ref
                class="inline-value multiline formula-source"
                prop:value=move || draft.get()
                spellcheck="false"
                on:scroll=move |_| sync_scroll()
                on:focus=move |_| focused.set(true)
                on:blur=move |_| {
                    focused.set(false);
                    ed.template_completion.set(None);
                    ed.template_signature.set(None);
                    let current = draft.get_untracked();
                    let stored = match value.get_untracked() {
                        Some(DataValue::String(s)) => s,
                        _ => String::new(),
                    };
                    if current != stored {
                        last_sent.set_value(Some(current.clone()));
                        on_commit.run(DataValue::String(current));
                    }
                }
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    draft.set(v.clone());
                    last_hovered.set_value(None);
                    sync_scroll();

                    if let Some(h) = timer.get_value() {
                        h.clear();
                        timer.set_value(None);
                    }
                    let handle = set_timeout_with_handle(move || {
                        let current = draft.get_untracked();
                        let stored = match value.get_untracked() {
                            Some(DataValue::String(s)) => s,
                            _ => String::new(),
                        };
                        let last = last_sent.get_value();
                        if ticknode_shared::formula::validate(&current).is_ok()
                            && last.as_deref() != Some(&current)
                            && current != stored
                        {
                            last_sent.set_value(Some(current.clone()));
                            on_commit.run(DataValue::String(current));
                        }
                    }, std::time::Duration::from_millis(400));
                    timer.set_value(handle.ok());

                    if ev.is_trusted() {
                        trigger_completion(false);
                    }
                }
                on:pointermove=move |ev| {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(textarea) = textarea_ref.get() {
                        let cx = ev.client_x() as f64;
                        let cy = ev.client_y() as f64;
                        let now = js_sys::Date::now();

                        let should_check = match last_pos.get_value() {
                            None => true,
                            Some((lx, ly)) => {
                                let dx = cx - lx;
                                let dy = cy - ly;
                                let moved_sq = dx * dx + dy * dy;
                                let elapsed = now - last_time.get_value();
                                moved_sq >= 4.0 || elapsed >= 60.0
                            }
                        };
                        if !should_check {
                            return;
                        }
                        last_pos.set_value(Some((cx, cy)));
                        last_time.set_value(now);

                        let current_text = draft.get_untracked();
                        let hovered = hover::hit_test_token(&textarea, cx, cy, &current_text);

                        if hovered != last_hovered.get_value() {
                            last_hovered.set_value(hovered.clone());
                            let desired_title = match hovered {
                                Some(token) => hover::resolve_hover_tooltip(
                                    ed,
                                    id,
                                    &token.name,
                                )
                                .unwrap_or_else(|| title.get_untracked()),
                                None => title.get_untracked(),
                            };
                            let current_title = textarea.get_attribute("title").unwrap_or_default();
                            if current_title != desired_title {
                                let _ = textarea.set_attribute("title", &desired_title);
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = ev;
                        let _ = &title;
                        let _ = &last_pos;
                        let _ = &last_time;
                        let _ = &last_auto_close;
                    }
                }
                on:pointerleave=move |_| {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(textarea) = textarea_ref.get() {
                        last_pos.set_value(None);
                        last_hovered.set_value(None);
                        let default_title = title.get_untracked();
                        let current_title = textarea.get_attribute("title").unwrap_or_default();
                        if current_title != default_title {
                            let _ = textarea.set_attribute("title", &default_title);
                        }
                    }
                }
                on:keydown=keydown
                on:keyup=move |_| crate::inline::formula_field::signature_help::update_signature_help(ed, id)
                on:click=move |_| crate::inline::formula_field::signature_help::update_signature_help(ed, id)
                on:wheel=|ev: web_sys::WheelEvent| ev.stop_propagation()
            ></textarea>
        </div>
    }
}
