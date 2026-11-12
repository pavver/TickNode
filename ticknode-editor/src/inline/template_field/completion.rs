//! Completion popup state, fixed layer, trigger evaluation, and item acceptance.

use leptos::prelude::*;
use ticknode_shared::NodeId;
use ticknode_shared::template::{Completion, CompletionItem, CompletionKind};

use crate::picker::{placement, viewport};
use crate::state::{Editor, use_editor};

/// Completion popup state anchored to the caret in client coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateCompletionState {
    pub node: NodeId,
    pub x: f64,
    pub caret_bottom: f64,
    pub caret_top: f64,
    pub completion: Completion,
    pub selected: usize,
}

/// Checks whether the user typed a trigger character: identifier, `.`, `|`,
/// a space after `is` / `{%`, or explicitly invoked completion via Ctrl+Space.
pub fn is_completion_trigger_char(prefix: &str, is_explicit: bool) -> bool {
    if is_explicit {
        return true;
    }
    let Some(last_char) = prefix.chars().last() else {
        return false;
    };
    if last_char.is_alphanumeric() || last_char == '_' || last_char == '.' || last_char == '|' {
        return true;
    }
    if last_char == ' ' {
        let trimmed = prefix.trim_end();
        if trimmed.ends_with("is") || trimmed.ends_with("{%") || trimmed.ends_with("{%-") {
            return true;
        }
    }
    false
}

/// Material Design icon glyph name for each completion item kind.
pub fn completion_kind_icon(kind: &CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Variable => "mdi-variable",
        CompletionKind::Field => "mdi-code-braces",
        CompletionKind::Filter => "mdi-filter-outline",
        CompletionKind::Test => "mdi-help-rhombus-outline",
        CompletionKind::Function => "mdi-function",
        CompletionKind::Keyword => "mdi-key-variant",
        CompletionKind::Attribute => "mdi-tag-outline",
    }
}

/// Accepts a completion item by finding the node's textarea in DOM, applying the splice,
/// positioning the caret, focusing the textarea, and dispatching an input event to notify Leptos.
pub fn accept_completion_item(
    ed: Editor,
    node: NodeId,
    item: &CompletionItem,
    range: std::ops::Range<usize>,
) {
    ed.template_completion.set(None);

    #[cfg(target_arch = "wasm32")]
    {
        use crate::inline::template_field::caret::{apply_completion, byte_to_utf16};
        use wasm_bindgen::JsCast;

        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let id_str = format!("template-textarea-{}", node.0);
            if let Some(textarea) = doc
                .get_element_by_id(&id_str)
                .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
            {
                let current = textarea.value();
                let (new_text, new_caret_byte) = apply_completion(&current, range, &item.insert);
                textarea.set_value(&new_text);
                let u16_caret = byte_to_utf16(&new_text, new_caret_byte) as u32;
                let _ = textarea.set_selection_range(u16_caret, u16_caret);
                let _ = textarea.focus();

                if let Some(pre) = doc.get_element_by_id(&format!("template-highlight-{}", node.0))
                {
                    pre.set_scroll_top(textarea.scroll_top());
                    pre.set_scroll_left(textarea.scroll_left());
                }

                // Dispatch an input event so that reactive listeners (draft, debounce timer) update
                if let Ok(ev) = web_sys::Event::new("input") {
                    let _ = textarea.dispatch_event(&ev);
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (node, item, range);
    }
}

/// Fixed overlay mounted at the app root next to `PickerLayer`.
#[component]
pub fn TemplateCompletionLayer() -> impl IntoView {
    let ed = use_editor();

    // Close on camera pan / zoom
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() {
            ed.template_completion.set(None);
        }
    });

    // Close on node removal
    Effect::new(move |_| {
        if let Some(state) = ed.template_completion.get()
            && ed.node_tracked(state.node).is_none()
        {
            ed.template_completion.set(None);
        }
    });

    move || {
        ed.template_completion
            .get()
            .map(|state| view! { <TemplateCompletionPopup state=state /> })
    }
}

#[component]
fn TemplateCompletionPopup(state: TemplateCompletionState) -> impl IntoView {
    let ed = use_editor();
    let node = state.node;
    let range = state.completion.range.clone();
    let items = state.completion.items.clone();
    let selected_index = state.selected;

    // Scroll active item into view
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(doc) = web_sys::window().and_then(|w| w.document())
            && let Some(el) = doc.get_element_by_id(&format!("tpl-opt-{selected_index}"))
        {
            let opts = web_sys::ScrollIntoViewOptions::new();
            opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
            el.scroll_into_view_with_scroll_into_view_options(&opts);
        }
    });

    let place = placement(
        (state.x, state.caret_top, state.x, state.caret_bottom),
        viewport(),
    );
    let vertical = if place.up {
        format!("bottom:{}px", place.edge)
    } else {
        format!("top:{}px", place.edge)
    };
    let style = format!(
        "left:{}px;width:{}px;{vertical};max-height:{}px",
        place.left, place.width, place.max_height
    );

    view! {
        <div class="picker-popup template-completion-popup" style=style on:pointerdown=|ev| ev.stop_propagation()>
            <div class="picker-list">
                {items.into_iter().take(50).enumerate().map(|(row, item)| {
                    let is_active = row == selected_index;
                    let item_clone = item.clone();
                    let range_clone = range.clone();
                    let icon = completion_kind_icon(&item.kind);
                    view! {
                        <div
                            id=format!("tpl-opt-{row}")
                            class=format!("picker-row template-completion-row{}", if is_active { " active" } else { "" })
                            on:mousemove=move |_| {
                                if selected_index != row {
                                    ed.template_completion.update(|opt| {
                                        if let Some(s) = opt { s.selected = row; }
                                    });
                                }
                            }
                            on:pointerdown=move |ev| {
                                ev.prevent_default();
                                accept_completion_item(ed, node, &item_clone, range_clone.clone());
                            }
                        >
                            <span class=format!("picker-icon mdi {icon}") aria-hidden="true"></span>
                            <span class="picker-row-label">{item.label}</span>
                            {(!item.detail.is_empty()).then(|| view! {
                                <span class="picker-row-detail template-detail">{item.detail}</span>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
