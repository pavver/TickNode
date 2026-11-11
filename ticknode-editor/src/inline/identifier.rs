//! Reuses the global searchable popup for typed identifier inputs and Match constants.
use super::*;
use crate::state::PickerState;
use wasm_bindgen::JsCast;

#[component]
pub(super) fn IdentifierValue(
    value: Signal<Option<DataValue>>,
    value_type: DataType,
    plugin: String,
    source: String,
    on_commit: Callback<Option<DataValue>>,
    optional: bool,
    title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let key = NodeId::generate();
    let state = RwSignal::new(ControlState::Idle);
    let raw = Signal::derive(move || value.with(|v| v.as_ref().map(text::format_value).unwrap_or_default()));
    let ty = value_type.clone();
    let pick = Callback::new(move |input: String| {
        if input.trim().is_empty() && optional {
            on_commit.run(None);
            return;
        }
        match i18n::parse_value_localized(&input, &ty) {
            Ok(value) => on_commit.run(Some(value)),
            Err(_) => flash(state, ControlState::Error),
        }
    });
    let toggle = move |ev: web_sys::MouseEvent| {
        if ed.picker.with_untracked(|p| p.as_ref().is_some_and(|p| p.node == key)) {
            ed.picker.set(None);
            return;
        }
        let Some(rect) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|e| e.get_bounding_client_rect())
        else {
            return;
        };
        ed.picker.set(Some(PickerState {
            node: key,
            property: String::new(),
            plugin: plugin.clone(),
            source: source.clone(),
            anchor: (rect.left(), rect.top(), rect.right(), rect.bottom()),
            on_pick: Some(pick),
            value: Some(raw),
        }));
    };
    on_cleanup(move || {
        if ed.picker.try_with_untracked(|p| p.as_ref().is_some_and(|p| p.node == key)).unwrap_or(false) {
            ed.picker.try_set(None);
        }
    });
    view! {
        <button class=move || format!("inline-value picker-field {}", state.get().class())
            title=move || title.get() on:pointerdown=|ev| ev.stop_propagation() on:click=toggle>
            <span class="picker-label">{move || value.with(|v| v.as_ref()
                .and_then(|v| i18n::identifier_text(v, &value_type)).unwrap_or_else(|| "—".into()))}</span>
        </button>
        {optional.then(|| view! { <button class="btn-icon ctl" title=move || t!("identifier-clear")
            on:pointerdown=|ev| ev.stop_propagation() on:click=move |_| on_commit.run(None)>
            <span class="mdi mdi-close" aria-hidden="true"></span>
        </button> })}
    }
}
