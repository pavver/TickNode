//! Completion evaluation and popup trigger for formula fields.

use crate::state::Editor;
#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;
use ticknode_shared::NodeId;

/// Checks whether the user typed a trigger character in formula editor:
/// identifier character, `.`, `:`, space after `(`, `,`, or operators, or explicit Ctrl+Space.
#[allow(dead_code)]
pub fn is_completion_trigger_char(prefix: &str, is_explicit: bool) -> bool {
    if is_explicit {
        return true;
    }
    let Some(last_char) = prefix.chars().last() else {
        return false;
    };
    if last_char.is_alphanumeric() || last_char == '_' || last_char == '.' || last_char == ':' {
        return true;
    }
    if last_char == ' ' {
        let trimmed = prefix.trim_end();
        if trimmed.ends_with('(')
            || trimmed.ends_with(',')
            || trimmed.ends_with('+')
            || trimmed.ends_with('-')
            || trimmed.ends_with('*')
            || trimmed.ends_with('/')
            || trimmed.ends_with('%')
            || trimmed.ends_with('^')
            || trimmed.ends_with('?')
            || trimmed.ends_with(':')
            || trimmed.ends_with("==")
            || trimmed.ends_with("!=")
            || trimmed.ends_with("&&")
            || trimmed.ends_with("||")
        {
            return true;
        }
    }
    false
}

/// Evaluates completion candidates for `node` and updates `ed.template_completion`.
pub fn request_completion(ed: Editor, node: NodeId, is_explicit: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::inline::template_field::caret::{measure_caret_screen_pos, utf16_to_byte};
        use crate::inline::template_field::completion::TemplateCompletionState;
        use wasm_bindgen::JsCast;

        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let id_str = format!("formula-textarea-{}", node.0);
        let Some(textarea) = doc
            .get_element_by_id(&id_str)
            .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        else {
            return;
        };

        let current_text = textarea.value();
        let u16_sel = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
        let caret_byte = utf16_to_byte(&current_text, u16_sel);
        let prefix = &current_text[..caret_byte];

        if !is_completion_trigger_char(prefix, is_explicit) {
            ed.template_completion.set(None);
            return;
        }

        let typed_inputs = crate::inline::template_field::preview::node_preview_inputs(ed, node)
            .into_iter()
            .map(|(name, _, ty)| (name, ty))
            .collect::<Vec<_>>();

        let completion =
            ticknode_shared::formula::complete(&current_text, caret_byte, &typed_inputs);

        if completion.items.is_empty() {
            ed.template_completion.set(None);
            return;
        }

        let (x, caret_bottom, caret_top) =
            measure_caret_screen_pos(&textarea, caret_byte, &current_text).unwrap_or_else(|| {
                let rect = textarea.get_bounding_client_rect();
                (rect.left() + 8.0, rect.bottom(), rect.top())
            });

        ed.template_completion.set(Some(TemplateCompletionState {
            node,
            x,
            caret_bottom,
            caret_top,
            completion,
            selected: 0,
        }));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (ed, node, is_explicit);
    }
}
