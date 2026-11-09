//! Compact duration field. Drafts stay local; only committed values go to the server.

use super::*;

pub(crate) mod units;
use units::{Unit, parse, presentation};

#[component]
pub(super) fn DurationValue(
    value: Signal<Option<DataValue>>,
    on_commit: Callback<Option<DataValue>>,
    optional: bool,
    title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let initial = value.get_untracked();
    let (initial_text, initial_unit) = presentation(initial.as_ref(), None);
    let draft = RwSignal::new(initial_text);
    let unit = RwSignal::new(initial_unit);
    let saved = RwSignal::new(initial);
    let state = RwSignal::new(ControlState::Idle);
    let error = RwSignal::new(None::<UiMessage>);
    let incoming = Memo::new(move |_| value.get());
    Effect::new(move |_| {
        let current = incoming.get();
        let (text, selected) = presentation(current.as_ref(), Some(unit.get_untracked()));
        draft.set(text);
        unit.set(selected);
        saved.set(current);
        error.set(None);
    });
    let restore = move || {
        let (Some(current), Some(selected)) = (saved.try_get_untracked(), unit.try_get_untracked()) else { return };
        let (text, selected) = presentation(current.as_ref(), Some(selected));
        draft.set(text);
        unit.set(selected);
    };
    let commit = move || {
        // Blur can arrive after node removal or a change of the Constant's type.
        let (Some(text), Some(selected), Some(current)) = (draft.try_get_untracked(), unit.try_get_untracked(), saved.try_get_untracked()) else { return };
        match parse(&text, selected) {
            Ok(None) if !optional => restore(),
            Ok(next) => {
                let (text, selected) = presentation(next.as_ref(), Some(selected));
                draft.set(text);
                unit.set(selected);
                error.set(None);
                if next != current {
                    saved.set(next.clone());
                    on_commit.run(next);
                }
            }
            Err(message) => {
                ed.log(LogKind::Error, format!("[ERROR] {message}"));
                error.set(Some(message));
                restore();
                flash(state, ControlState::Error);
            }
        }
    };
    view! {
        <span class="duration-value" on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()>
            <input
                type="text"
                inputmode="text"
                class=move || format!("inline-value duration-number {}", state.get().class())
                placeholder=move || t!("hint-duration")
                aria-label=move || title.get()
                aria-invalid=move || error.with(Option::is_some).to_string()
                title=move || error.with(|e| e.as_ref().map(i18n::ui_text)).unwrap_or_else(|| {
                    format!("{}\n{}", title.get(), t!("duration-input-hint"))
                })
                spellcheck="false"
                prop:value=move || draft.get()
                on:input=move |ev| { draft.set(event_target_value(&ev)); error.set(None); }
                on:focus=move |ev| {
                    use wasm_bindgen::JsCast;
                    if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { input.select(); }
                }
                // Moving into the unit select commits both fields in its change/blur handler.
                on:blur=move |ev| {
                    use wasm_bindgen::JsCast;
                    let inside = ev.related_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        .is_some_and(|el| el.get_attribute("class").is_some_and(|c| c.split_whitespace().any(|c| c == "duration-unit")));
                    if !inside { commit(); }
                }
                on:keydown=move |ev| {
                    use wasm_bindgen::JsCast;
                    match ev.key().as_str() {
                        "Enter" | "Escape" => {
                            ev.prevent_default();
                            ev.stop_propagation();
                            if ev.key() == "Escape" { restore(); error.set(None); }
                            if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) { let _ = input.blur(); }
                        }
                        _ => {}
                    }
                }
            />
            <select
                class="inline-value select duration-unit"
                aria-label=move || t!("duration-unit-title")
                title=move || t!("duration-unit-title")
                prop:value=move || unit.get().id()
                on:change=move |ev| {
                    if let Some(next) = Unit::from_id(&event_target_value(&ev)) { unit.set(next); commit(); }
                }
                on:blur=move |_| commit()
            >
                {Unit::ALL.into_iter().map(|u| view! { <option value=u.id()>{move || u.label()}</option> }).collect_view()}
            </select>
        </span>
    }
}
