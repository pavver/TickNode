//! Number and localized unit controls for canonical physical values.
use super::*;
use ticknode_shared::quantity::QuantityKind;
pub(crate) mod units;

#[component]
pub(super) fn QuantityValue(
    value: Signal<Option<DataValue>>,
    kind: QuantityKind,
    on_commit: Callback<Option<DataValue>>,
    optional: bool,
    title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let kind = StoredValue::new(kind);
    let rule = move || kind.with_value(|q| {
        let rules = i18n::display_rules();
        let name = ticknode_shared::quantity::catalog().iter().find(|d| d.quantity() == Some(q)).map_or("", |d| d.name.as_str());
        rules.effective(name, q)
    });
    let initial = value.get_untracked();
    let (text, selected) = kind.with_value(|q| units::presentation(initial.as_ref(), q, None, &i18n::decimal_separator(), &rule()));
    let last_shown = RwSignal::new((text.clone(), selected.clone()));
    let draft = RwSignal::new(text);
    let unit = RwSignal::new(selected);
    let saved = RwSignal::new(initial);
    let error = RwSignal::new(None::<UiMessage>);
    let state = RwSignal::new(ControlState::Idle);
    Effect::new(move |_| {
        let current = value.get();
        let _ = ed.server_settings.get();
        let decimal = i18n::decimal_separator();
        let (text, selected) = kind.with_value(|q| units::presentation(current.as_ref(), q, None, &decimal, &rule()));
        last_shown.set((text.clone(), selected.clone()));
        draft.set(text); unit.set(selected); saved.set(current); error.set(None);
    });
    let restore = move || {
        let (Some(current), Some(selected)) = (saved.try_get_untracked(), unit.try_get_untracked()) else { return };
        let (text, _) = kind.with_value(|q| units::presentation(current.as_ref(), q, Some(&selected), &i18n::decimal_separator(), &rule()));
        last_shown.set((text.clone(), selected));
        draft.set(text);
    };
    let commit = move || {
        let (Some(text), Some(selected), Some(current)) = (draft.try_get_untracked(), unit.try_get_untracked(), saved.try_get_untracked()) else { return };
        if last_shown.try_get_untracked().as_ref() == Some(&(text.clone(), selected.clone())) { return; }
        let parsed = kind.with_value(|q| units::parse(&text, &selected, q, &title.get_untracked()));
        match parsed {
            Ok(None) if !optional => restore(),
            Ok(next) => {
                let (text, _) = kind.with_value(|q| units::presentation(next.as_ref(), q, Some(&selected), &i18n::decimal_separator(), &rule()));
                last_shown.set((text.clone(), selected.clone()));
                draft.set(text); error.set(None);
                if next != current { saved.set(next.clone()); on_commit.run(next); }
            }
            Err(message) => {
                ed.log(LogKind::Error, format!("[ERROR] {message}"));
                error.set(Some(message)); restore(); flash(state, ControlState::Error);
            }
        }
    };
    let options = kind.with_value(|q| q.units.clone());
    view! {
        <span class="duration-value" on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()>
            <input type="text" inputmode="text" spellcheck="false"
                class=move || format!("inline-value duration-number {}", state.get().class())
                prop:value=move || draft.get()
                aria-label=move || title.get()
                aria-invalid=move || error.with(Option::is_some).to_string()
                title=move || error.with(|e| e.as_ref().map(i18n::ui_text)).unwrap_or_else(|| title.get())
                on:input=move |ev| { draft.set(event_target_value(&ev)); error.set(None); }
                on:focus=move |ev| {
                    use wasm_bindgen::JsCast;
                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { el.select(); }
                }
                on:blur=move |ev| {
                    use wasm_bindgen::JsCast;
                    let inside = ev.related_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        .is_some_and(|e| e.class_list().contains("duration-unit"));
                    if !inside { commit(); }
                }
                on:keydown=move |ev| {
                    use wasm_bindgen::JsCast;
                    if matches!(ev.key().as_str(), "Enter" | "Escape") {
                        ev.prevent_default(); ev.stop_propagation();
                        if ev.key() == "Escape" { restore(); error.set(None); }
                        if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) { let _ = el.blur(); }
                    }
                }
            />
            <select class="inline-value select duration-unit"
                prop:value=move || unit.get()
                aria-label=move || t!("quantity-unit-title")
                title=move || t!("quantity-unit-title")
                on:change=move |ev| { unit.set(event_target_value(&ev)); commit(); }
                on:blur=move |_| commit()>
                {options.into_iter().map(|u| {
                    let id = u.id.clone(); let symbol = u.symbol.clone();
                    view! { <option value=u.id>{move || i18n::unit_symbol(&id).unwrap_or_else(|| symbol.clone())}</option> }
                }).collect_view()}
            </select>
        </span>
    }
}
