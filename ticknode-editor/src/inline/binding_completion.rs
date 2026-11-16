//! Inline bindings reuse the template popup and namespace tree completion.
use crate::state::Editor;
use leptos::prelude::*;
use ticknode_shared::{template::CompletionItem, *};

#[component]
pub(super) fn BindingCompletion(
    ed: Editor,
    node: NodeId,
    pin: PinId,
    children: Children,
) -> impl IntoView {
    view! {
        <span class="inline-binding" data-binding-node=node.0.to_string() data-binding-pin=pin.0
            on:input=move |ev| {
                #[cfg(target_arch = "wasm32")]
                if ev.is_trusted() {
                    use wasm_bindgen::JsCast;
                    if let Some(element) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                        request(ed, node, pin, &element);
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                let _ = ev;
            }
            on:keydown:capture=move |ev| keydown(ed, node, pin, ev)
            on:focusout=move |_| {
                if ed.template_completion.with_untracked(|s| s.as_ref().is_some_and(|s| s.node == node && s.binding == Some(pin))) {
                    ed.template_completion.set(None);
                }
            }
        >{children()}</span>
    }
}

fn keydown(ed: Editor, node: NodeId, pin: PinId, ev: web_sys::KeyboardEvent) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if (ev.ctrl_key() || ev.meta_key()) && ev.code() == "Space" {
            ev.prevent_default();
            ev.stop_immediate_propagation();
            if let Some(element) = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            {
                request(ed, node, pin, &element);
            }
            return;
        }
        let state = ed.template_completion.get_untracked();
        let Some(state) = state.filter(|s| s.node == node && s.binding == Some(pin)) else {
            return;
        };
        let count = state.completion.items.len();
        match ev.key().as_str() {
            "ArrowDown" | "ArrowUp" if count > 0 => {
                let down = ev.key() == "ArrowDown";
                ed.template_completion.update(|state| {
                    if let Some(state) = state {
                        state.selected = if down {
                            (state.selected + 1) % count
                        } else {
                            (state.selected + count - 1) % count
                        };
                    }
                });
            }
            "Enter" | "Tab" => {
                if let Some(item) = state.completion.items.get(state.selected) {
                    accept(ed, node, pin, item, state.completion.range);
                }
            }
            "Escape" => ed.template_completion.set(None),
            "ArrowLeft" | "ArrowRight" | "Home" | "End" => {
                ed.template_completion.set(None);
                return;
            }
            _ => return,
        }
        ev.prevent_default();
        ev.stop_immediate_propagation();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (ed, node, pin, ev);
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn request(ed: Editor, node: NodeId, pin: PinId, element: &web_sys::Element) {
    #[cfg(target_arch = "wasm32")]
    {
        use super::template_field::{caret::utf16_to_byte, completion::TemplateCompletionState};
        use wasm_bindgen::JsCast;
        let (text, selection) = if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>()
        {
            let _ = input.set_attribute("id", &format!("binding-{}-{}", node.0, pin.0));
            (input.value(), input.selection_start().ok().flatten())
        } else if let Some(input) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            let _ = input.set_attribute("id", &format!("binding-{}-{}", node.0, pin.0));
            (input.value(), input.selection_start().ok().flatten())
        } else {
            return;
        };
        let caret = utf16_to_byte(&text, selection.unwrap_or(0) as usize);
        // Quoted strings remain literals, and multi-line text is not a binding reference.
        if text.starts_with('"') || text.contains('\n') {
            ed.template_completion.set(None);
            return;
        }
        ed.request_value_fields(&text, false);
        let variables = ed.variables.with(|vars| {
            vars.values()
                .filter(|v| {
                    v.key.scope != VariableScope::Sheet || v.key.sheet == ed.open_sheet.get()
                })
                .cloned()
                .collect::<Vec<_>>()
        });
        let mut completion = ed.with_value_tree(|ctx| {
            ed.types.with(|types| {
                formula::complete::complete_with_catalog(
                    &template::CompletionInput {
                        source: &text,
                        caret,
                        inputs: &[],
                        types,
                        variables: &variables,
                        now: None,
                        labels: Some(&crate::i18n::UiQuantityLabels),
                    },
                    ctx,
                )
            })
        });
        completion.items.retain(|item| {
            matches!(
                item.kind,
                template::CompletionKind::Variable | template::CompletionKind::Field
            )
        });
        completion.items.truncate(50);
        let rect = element.get_bounding_client_rect();
        ed.template_completion
            .set(
                (!completion.items.is_empty()).then_some(TemplateCompletionState {
                    node,
                    binding: Some(pin),
                    x: rect.left(),
                    caret_bottom: rect.bottom(),
                    caret_top: rect.top(),
                    completion,
                    selected: 0,
                }),
            );
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (ed, node, pin, element);
}

pub(crate) fn accept(
    ed: Editor,
    node: NodeId,
    pin: PinId,
    item: &CompletionItem,
    range: std::ops::Range<usize>,
) {
    ed.template_completion.set(None);
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(element) = doc.get_element_by_id(&format!("binding-{}-{}", node.0, pin.0)) else {
            return;
        };
        if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
            let (start, end, caret, text, _) =
                super::undoable::plan_completion(&input.value(), range, &item.insert, item.caret);
            let _ = input.focus();
            let _ = input.set_selection_range(start, end);
            let applied = doc.dyn_ref::<web_sys::HtmlDocument>().is_some_and(|doc| {
                doc.exec_command_with_show_ui_and_value("insertText", false, &item.insert)
                    .unwrap_or(false)
            });
            if !applied {
                input.set_value(&text);
                let options = web_sys::EventInit::new();
                options.set_bubbles(true);
                if let Ok(event) = web_sys::Event::new_with_event_init_dict("input", &options) {
                    let _ = input.dispatch_event(&event);
                }
            }
            let _ = input.set_selection_range(caret, caret);
        } else if let Some(input) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            let (start, end, caret, _, _) =
                super::undoable::plan_completion(&input.value(), range, &item.insert, item.caret);
            super::undoable::replace_range(input, start, end, &item.insert, caret);
        }
        ed.template_completion.set(None);
        if item.insert.ends_with('.') {
            request(ed, node, pin, &element);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (node, pin, item, range);
}
