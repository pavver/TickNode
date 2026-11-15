//! Signature help evaluation for formula fields.

use crate::state::Editor;
#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;
use ticknode_shared::NodeId;

/// Evaluates signature help for `node` and updates `ed.template_signature`.
pub fn update_signature_help(ed: Editor, node: NodeId) {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::inline::template_field::caret::{measure_caret_screen_pos, utf16_to_byte};
        use crate::inline::template_field::signature_help::TemplateSignatureState;
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

        if let Some(help) = ticknode_shared::formula::signature_at(&current_text, caret_byte) {
            let (x, caret_bottom, caret_top) =
                measure_caret_screen_pos(&textarea, caret_byte, &current_text).unwrap_or_else(
                    || {
                        let rect = textarea.get_bounding_client_rect();
                        (rect.left() + 8.0, rect.bottom(), rect.top())
                    },
                );

            ed.template_signature.set(Some(TemplateSignatureState {
                node,
                x,
                caret_bottom,
                caret_top,
                help,
            }));
            return;
        }

        ed.template_signature.set(None);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (ed, node);
    }
}
