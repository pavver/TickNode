//! In-place template property editing with completion, live syntax validation, and preview.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::state::use_editor;

pub mod auto_close;
pub mod caret;
pub mod completion;
pub mod highlight;
pub mod hover;
pub mod preview;
pub mod signature_help;

#[cfg(test)]
mod tests;

pub use completion::{TemplateCompletionLayer, TemplateCompletionState};
pub use highlight::TemplateHighlightLayer;
pub use preview::TemplateStatusRow;
pub use signature_help::TemplateSignatureState;

/// Reactive context storing the current draft of the template being edited on a node.
#[derive(Clone, Copy)]
pub struct TemplateDraft(pub RwSignal<String>);

/// Textarea editor for the `template` property of `text/template` nodes.
#[component]
pub fn TemplateField(
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
    let draft = use_context::<TemplateDraft>()
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
    let last_hovered = StoredValue::<Option<hover::HoverToken>>::new(None);
    let last_auto_close = StoredValue::<Option<auto_close::AutoCloseState>>::new(None);

    on_cleanup(move || {
        if let Some(h) = timer.get_value() {
            h.clear();
        }
    });

    let trigger_completion = move |_is_explicit: bool| {
        completion::request_completion(ed, id, _is_explicit);
        signature_help::update_signature_help(ed, id);
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
                "PageDown" if count > 0 => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    ed.template_completion.update(|opt| {
                        if let Some(s) = opt {
                            s.selected = (s.selected + 8).min(count - 1);
                        }
                    });
                    return;
                }
                "PageUp" if count > 0 => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    ed.template_completion.update(|opt| {
                        if let Some(s) = opt {
                            s.selected = s.selected.saturating_sub(8);
                        }
                    });
                    return;
                }
                "Enter" | "Tab" => {
                    ev.prevent_default();
                    ev.stop_propagation();
                    let chosen = ed.template_completion.with_untracked(|c| {
                        c.as_ref().and_then(|s| {
                            s.completion
                                .items
                                .get(s.selected)
                                .cloned()
                                .map(|item| (item, s.completion.range.clone()))
                        })
                    });
                    if let Some((item, range)) = chosen {
                        completion::accept_completion_item(ed, id, &item, range);
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
                // Moving the caret leaves the completion range behind.
                "ArrowLeft" | "ArrowRight" | "Home" | "End" => {
                    ed.template_completion.set(None);
                    ed.template_signature.set(None);
                    signature_help::update_signature_help(ed, id);
                }
                _ => {}
            }
        }

        if key == "Escape" {
            if ed.template_signature.with_untracked(|s| s.is_some()) {
                ev.prevent_default();
                ev.stop_propagation();
                ed.template_signature.set(None);
                return;
            }
            ev.prevent_default();
            ev.stop_propagation();
            let stored = match value.get_untracked() {
                Some(DataValue::String(s)) => s,
                _ => String::new(),
            };
            draft.set(stored);
            if let Some(el) = textarea_ref.get() {
                let _ = el.blur();
            }
            return;
        }

        if key == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
            ev.prevent_default();
            ev.stop_propagation();
            if let Some(el) = textarea_ref.get() {
                let _ = el.blur();
            }
            return;
        }

        let action = {
            #[cfg(target_arch = "wasm32")]
            if let Some(textarea) = textarea_ref.get() {
                use caret::utf16_to_byte;
                let current_text = draft.get_untracked();
                let u16_start = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
                let u16_end = textarea.selection_end().ok().flatten().unwrap_or(0) as usize;
                let start_byte = utf16_to_byte(&current_text, u16_start);
                let end_byte = utf16_to_byte(&current_text, u16_end);
                auto_close::decide_typing(
                    &current_text,
                    start_byte..end_byte,
                    &key,
                    last_auto_close.get_value(),
                )
            } else {
                auto_close::AutoCloseAction::None
            }
            #[cfg(not(target_arch = "wasm32"))]
            auto_close::AutoCloseAction::None
        };

        match action {
            auto_close::AutoCloseAction::None => {
                if key != "Shift" && key != "Control" && key != "Alt" && key != "Meta" {
                    last_auto_close.set_value(None);
                }
            }
            auto_close::AutoCloseAction::Insert {
                range,
                text,
                new_caret,
                state,
            } => {
                ev.prevent_default();
                ev.stop_propagation();
                last_auto_close.set_value(Some(state));

                #[cfg(target_arch = "wasm32")]
                if let Some(textarea) = textarea_ref.get() {
                    use caret::{apply_completion, byte_to_utf16};
                    let current = textarea.value();
                    let (new_text, _) = apply_completion(&current, range, text, None);
                    textarea.set_value(&new_text);
                    let u16_caret = byte_to_utf16(&new_text, new_caret) as u32;
                    let _ = textarea.set_selection_range(u16_caret, u16_caret);
                    draft.set(new_text);
                    sync_scroll();
                    if let Ok(ev) = web_sys::Event::new("input") {
                        let _ = textarea.dispatch_event(&ev);
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (range, text, new_caret);
            }
            auto_close::AutoCloseAction::SkipChar { new_caret } => {
                ev.prevent_default();
                ev.stop_propagation();
                last_auto_close.set_value(None);

                #[cfg(target_arch = "wasm32")]
                if let Some(textarea) = textarea_ref.get() {
                    use caret::byte_to_utf16;
                    let current = textarea.value();
                    let u16_caret = byte_to_utf16(&current, new_caret) as u32;
                    let _ = textarea.set_selection_range(u16_caret, u16_caret);
                }
                #[cfg(not(target_arch = "wasm32"))]
                let _ = new_caret;
            }
            auto_close::AutoCloseAction::DeletePair {
                range,
                replacement,
                new_caret,
            } => {
                ev.prevent_default();
                ev.stop_propagation();
                last_auto_close.set_value(None);

                #[cfg(target_arch = "wasm32")]
                if let Some(textarea) = textarea_ref.get() {
                    use caret::{apply_completion, byte_to_utf16};
                    let current = textarea.value();
                    let (new_text, _) = apply_completion(&current, range, replacement, None);
                    textarea.set_value(&new_text);
                    let u16_caret = byte_to_utf16(&new_text, new_caret) as u32;
                    let _ = textarea.set_selection_range(u16_caret, u16_caret);
                    draft.set(new_text);
                    sync_scroll();
                    if let Ok(ev) = web_sys::Event::new("input") {
                        let _ = textarea.dispatch_event(&ev);
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (range, replacement, new_caret);
            }
        }
    };

    let has_syntax_error =
        Memo::new(move |_| draft.with(|d| ticknode_shared::template::validate(d).is_err()));

    view! {
        <div class="template-editor">
            <TemplateHighlightLayer
                node_id=id
                pre_ref=pre_ref
                draft=Signal::derive(move || draft.get())
                inputs=input_names
            />
            <textarea
                id=format!("template-textarea-{}", id.0)
                node_ref=textarea_ref
                class="inline-value multiline template-source"
                class:st-error=move || has_syntax_error.get()
                title=move || title.get()
                spellcheck="false"
                prop:value=move || draft.get()
                on:pointerdown=move |ev| {
                    crate::match_editor::stop_on_controls(ev);
                    last_auto_close.set_value(None);
                    if ed.template_completion.with_untracked(|c| c.is_some()) {
                        ed.template_completion.set(None);
                    ed.template_signature.set(None);
                    }
                }
                on:focus=move |_| focused.set(true)
                on:blur=move |_| {
                    focused.set(false);
                    last_pos.set_value(None);
                    last_hovered.set_value(None);
                    last_auto_close.set_value(None);
                    if let Some(h) = timer.get_value() {
                        h.clear();
                        timer.set_value(None);
                    }
                    ed.template_completion.set(None);
                    ed.template_signature.set(None);

                    let current = draft.get_untracked();
                    let stored = match value.get_untracked() {
                        Some(DataValue::String(s)) => s,
                        _ => String::new(),
                    };
                    if current != stored {
                        // The server echo resyncs the draft; resetting here would flash the old text.
                        last_sent.set_value(Some(current.clone()));
                        on_commit.run(DataValue::String(current));
                    } else {
                        draft.set(stored);
                    }
                }
                on:scroll=move |_| sync_scroll()
                on:input=move |ev| {
                    let new_val = event_target_value(&ev);
                    draft.set(new_val.clone());
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
                        if ticknode_shared::template::validate(&current).is_ok()
                            && last.as_deref() != Some(&current)
                            && current != stored
                        {
                            last_sent.set_value(Some(current.clone()));
                            on_commit.run(DataValue::String(current));
                        }
                    }, std::time::Duration::from_millis(400));
                    timer.set_value(handle.ok());

                    // Accepting an item dispatches a synthetic input event: do not reopen the popup for it.
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
                            Some(token) => hover::resolve_hover_tooltip_with_context(
                                ed,
                                id,
                                &token.path,
                                Some(token.range.clone()),
                                Some(&current_text),
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
                    let _ = &last_pos;
                    let _ = &last_time;
                    let _ = &last_hovered;
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
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = &last_pos;
                    let _ = &last_hovered;
                }
            }
            on:keydown=keydown
            on:keyup=move |_| signature_help::update_signature_help(ed, id)
            on:click=move |_| signature_help::update_signature_help(ed, id)
            on:wheel=|ev: web_sys::WheelEvent| ev.stop_propagation()
        ></textarea>
        </div>
    }
}
