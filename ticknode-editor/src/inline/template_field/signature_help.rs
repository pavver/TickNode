//! Signature help overlay component and trigger evaluation for template fields.

use leptos::prelude::*;
use ticknode_shared::NodeId;
use ticknode_shared::template::complete::SignatureHelp;

use crate::state::Editor;

/// Active signature help anchored to the caret in client coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSignatureState {
    pub node: NodeId,
    pub x: f64,
    pub caret_bottom: f64,
    pub caret_top: f64,
    pub help: SignatureHelp,
}

/// Evaluates signature help for `node` and updates `ed.template_signature`.
pub fn update_signature_help(ed: Editor, node: NodeId) {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::inline::template_field::caret::{measure_caret_screen_pos, utf16_to_byte};
        use wasm_bindgen::JsCast;

        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            ed.template_signature.set(None);
            return;
        };
        let id_str = format!("template-textarea-{}", node.0);
        let Some(textarea) = doc
            .get_element_by_id(&id_str)
            .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        else {
            ed.template_signature.set(None);
            return;
        };

        let current_text = textarea.value();
        let u16_start = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
        let caret_byte = utf16_to_byte(&current_text, u16_start);

        if let Some(help) = ticknode_shared::template::signature_at(&current_text, caret_byte)
            && let Some((x, caret_bottom, caret_top)) =
                measure_caret_screen_pos(&textarea, caret_byte, &current_text)
        {
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

/// Fixed overlay popup showing the active function/filter/test signature and active parameter help.
#[component]
pub fn TemplateSignaturePopup(state: TemplateSignatureState) -> impl IntoView {
    let help = state.help;
    let signature = help.signature.clone();
    let active_param_idx = help.active_param;
    let params = help.params.clone();

    let (before_param, active_param_str, after_param) =
        if let Some(param) = params.get(active_param_idx) {
            let name = &param.name;
            if let Some(pos) = signature.find(name.as_str()) {
                let mut start = pos;
                let mut end = pos + name.len();
                if start > 0
                    && signature.as_bytes()[start - 1] == b'['
                    && signature.as_bytes().get(end) == Some(&b']')
                {
                    start -= 1;
                    end += 1;
                }
                (
                    signature[..start].to_string(),
                    Some(signature[start..end].to_string()),
                    signature[end..].to_string(),
                )
            } else {
                (signature, None, String::new())
            }
        } else {
            (signature, None, String::new())
        };

    let desc_msg = params
        .get(active_param_idx)
        .map(|p| &p.help)
        .unwrap_or(&help.help);
    let desc_text = crate::i18n::ui_text(desc_msg);

    let vp = crate::picker::viewport();
    let is_up = state.caret_top > 60.0;
    let vertical_style = if is_up {
        format!("bottom:{}px", (vp.1 - state.caret_top + 4.0).max(8.0))
    } else {
        format!("top:{}px", (state.caret_bottom + 4.0).max(8.0))
    };
    let left = state.x.clamp(16.0, (vp.0 - 240.0).max(16.0));
    let style = format!("left:{left}px;{vertical_style}");

    view! {
        <div class="template-signature-popup" style=style>
            <div class="template-signature-code">
                {before_param}
                {active_param_str.map(|s| view! {
                    <strong class="template-signature-param-active">{s}</strong>
                })}
                {after_param}
            </div>
            {(!desc_text.is_empty()).then(|| view! {
                <div class="template-signature-desc">
                    {desc_text}
                </div>
            })}
        </div>
    }
}
