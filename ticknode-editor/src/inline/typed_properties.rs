//! Widgets for schema-defined text properties; persistence remains textual.
use super::*;
use ticknode_shared::property::{normalize_property_value, parse_property_duration};

#[component]
pub(super) fn TypedProperty(
    kind: PropertyKind,
    owner: String,
    name: String,
    value: Signal<Option<DataValue>>,
    title: Signal<String>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let raw = Signal::derive(move || match value.get() {
        Some(DataValue::String(s)) => s,
        _ => String::new(),
    });
    match kind {
        PropertyKind::Duration { signed } => {
            view! { <DurationProperty raw=raw signed=signed title=title on_commit=on_commit /> }
                .into_any()
        }
        PropertyKind::Time => {
            view! { <TimeProperty raw=raw title=title on_commit=on_commit /> }.into_any()
        }
        PropertyKind::MultiSelect(options) => {
            let all = StoredValue::new(options.clone());
            view! {
                <span class="property-chips" role="group" aria-label=move || title.get()>
                    {options.into_iter().map(|(option, label)| {
                        let selected_key = option.clone();
                        let label_key = option.clone();
                        let owner = owner.clone();
                        let name = name.clone();
                        let selected = Signal::derive(move || raw.with(|s| s.split(',').any(|v| v.trim() == selected_key)));
                        let short_owner = owner.clone();
                        let short_name = name.clone();
                        let short_key = label_key.clone();
                        let short_label = label.clone();
                        let short = Signal::derive(move || i18n::property_option_short_label(&short_owner, &short_name, &short_key, &short_label));
                        let text = Signal::derive(move || i18n::property_option_label(&owner, &name, &label_key, &label));
                        view! {
                            <button type="button" class="property-chip" class:selected=move || selected.get() aria-pressed=move || selected.get().to_string()
                                title=move || text.get()
                                on:click=move |_| {
                                    let current = raw.get_untracked();
                                    let active = current.split(',').any(|v| v.trim() == option);
                                    let next = all.with_value(|options| options.iter().filter(|(key, _)| {
                                        if *key == option { !active } else { current.split(',').any(|v| v.trim() == key) }
                                    }).map(|(key, _)| key.as_str()).collect::<Vec<_>>().join(","));
                                    on_commit.run(next.into());
                                }>{move || short.get()}</button>
                        }
                    }).collect_view()}
                </span>
            }.into_any()
        }
        _ => ().into_any(),
    }
}

#[component]
fn DurationProperty(
    raw: Signal<String>,
    signed: bool,
    title: Signal<String>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let negative = RwSignal::new(raw.get_untracked().trim().starts_with('-'));
    Effect::new(move |_| negative.set(raw.get().trim().starts_with('-')));
    let magnitude = Signal::derive(move || {
        let text = raw.get();
        let text = text.trim().trim_start_matches(['-', '+']);
        parse_property_duration(text).map(DataValue::Duration)
    });
    let send = move |value: Option<DataValue>| {
        on_commit.run(duration_property_text(value.as_ref(), negative.get_untracked()).into());
    };
    view! {
        <span class="duration-property">
            {signed.then(|| view! {
                <button type="button" class="mini-btn duration-sign"
                    title=move || if negative.get() { t!("property-duration-earlier") } else { t!("property-duration-later") }
                    aria-label=move || if negative.get() { t!("property-duration-earlier") } else { t!("property-duration-later") }
                    on:click=move |_| {
                        negative.update(|n| *n = !*n);
                        if magnitude.get_untracked().is_some() { send(magnitude.get_untracked()); }
                    }>{move || if negative.get() { "−" } else { "+" }}</button>
            })}
            <duration::DurationValue value=magnitude optional=true wall_only=true default_minutes=signed title=title on_commit=Callback::new(send) />
        </span>
    }
}

#[component]
fn TimeProperty(
    raw: Signal<String>,
    title: Signal<String>,
    on_commit: Callback<DataValue>,
) -> impl IntoView {
    let draft = RwSignal::new(raw.get_untracked());
    let error = RwSignal::new(None::<UiMessage>);
    Effect::new(move |_| {
        draft.set(raw.get());
        error.set(None);
    });
    let commit = move || {
        let Some(text) = draft.try_get_untracked() else {
            return;
        };
        match normalize_property_value(&PropertyKind::Time, &text.into()) {
            Ok(value) => {
                error.set(None);
                on_commit.run(value);
            }
            Err(message) => {
                draft.set(raw.get_untracked());
                error.set(Some(message));
            }
        }
    };
    view! {
        <input type="text" class="inline-value" class:st-error=move || error.with(Option::is_some)
            placeholder=move || t!("property-time-placeholder")
            aria-label=move || title.get() aria-invalid=move || error.with(Option::is_some).to_string()
            title=move || error.with(|e| e.as_ref().map(i18n::ui_text)).unwrap_or_else(|| title.get())
            prop:value=move || draft.get()
            on:input=move |ev| { draft.set(event_target_value(&ev)); error.set(None); }
            on:blur=move |_| commit()
            on:keydown=move |ev| {
                use wasm_bindgen::JsCast;
                if matches!(ev.key().as_str(), "Enter" | "Escape") {
                    ev.prevent_default(); ev.stop_propagation();
                    if ev.key() == "Escape" { draft.set(raw.get_untracked()); error.set(None); }
                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) { let _ = el.blur(); }
                }
            } />
    }
}

/// Preserve sub-millisecond offsets without converting through a floating point number.
fn duration_property_text(value: Option<&DataValue>, negative: bool) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let text = match value {
        DataValue::Duration(d) if !d.subsec_nanos().is_multiple_of(1_000_000) => {
            format!("{}.{:09}s", d.as_secs(), d.subsec_nanos())
        }
        _ => text::format_value(value),
    };
    if negative { format!("-{text}") } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_duration_properties_preserve_exact_text_and_empty_values() {
        use std::time::Duration;
        for duration in [
            Duration::from_nanos(1),
            Duration::from_micros(1234),
            Duration::from_millis(1500),
            Duration::from_secs(3600),
        ] {
            let value = DataValue::Duration(duration);
            let positive = duration_property_text(Some(&value), false);
            assert_eq!(parse_property_duration(&positive), Some(duration));
            assert_eq!(
                duration_property_text(Some(&value), true),
                format!("-{positive}")
            );
        }
        assert_eq!(duration_property_text(None, true), "");
    }
}
