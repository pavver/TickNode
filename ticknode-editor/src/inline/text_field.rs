//! String drafts preserve whitespace and newlines across single-line and multiline presentation.
use super::*;

#[derive(Debug, PartialEq)]
enum TextKey { Commit, Cancel, Insert }

fn text_key(key: &str, multiline: bool, modifier: bool) -> TextKey {
    match key {
        "Escape" => TextKey::Cancel,
        "Enter" if !multiline || modifier => TextKey::Commit,
        _ => TextKey::Insert,
    }
}

#[component]
pub(super) fn TextField(
    value: Signal<Option<DataValue>>,
    title: Signal<String>,
    multiline: Signal<bool>,
    on_commit: Callback<Option<DataValue>>,
) -> impl IntoView {
    let shown = Memo::new(move |_| match value.get() {
        Some(DataValue::String(s)) => s,
        _ => String::new(),
    });
    let draft = RwSignal::new(shown.get_untracked());
    Effect::new(move |_| draft.set(shown.get()));
    let commit = move || {
        let (Some(text), Some(saved)) = (draft.try_get_untracked(), shown.try_get_untracked()) else { return };
        if text != saved { on_commit.run(Some(DataValue::String(text))); }
    };
    let keydown = move |ev: web_sys::KeyboardEvent| {
        use wasm_bindgen::JsCast;
        match text_key(&ev.key(), multiline.get_untracked(), ev.ctrl_key() || ev.meta_key()) {
            TextKey::Insert => return,
            TextKey::Cancel => { draft.set(shown.get_untracked()); }
            TextKey::Commit => {}
        }
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) { let _ = el.blur(); }
    };
    move || if multiline.get() {
        view! {
            <textarea
                class="inline-value multiline"
                title=move || title.get()
                spellcheck="false"
                prop:value=move || draft.get()
                on:pointerdown=crate::match_editor::stop_on_controls
                on:input=move |ev| draft.set(event_target_value(&ev))
                on:wheel=|ev: web_sys::WheelEvent| ev.stop_propagation()
                on:blur=move |_| commit()
                on:keydown=keydown
            ></textarea>
        }.into_any()
    } else {
        view! {
            <input
                type="text"
                class="inline-value"
                title=move || title.get()
                placeholder=move || i18n::type_hint(&DataType::String)
                spellcheck="false"
                prop:value=move || draft.get()
                on:pointerdown=crate::match_editor::stop_on_controls
                on:input=move |ev| draft.set(event_target_value(&ev))
                on:focus=move |ev| {
                    use wasm_bindgen::JsCast;
                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { el.select(); }
                }
                on:blur=move |_| commit()
                on:keydown=keydown
            />
        }.into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiline_enter_inserts_and_modified_enter_commits() {
        assert_eq!(text_key("Enter", true, false), TextKey::Insert);
        assert_eq!(text_key("Enter", true, true), TextKey::Commit);
        assert_eq!(text_key("Enter", false, false), TextKey::Commit);
        assert_eq!(text_key("Escape", true, true), TextKey::Cancel);
        assert_eq!(text_key("a", true, false), TextKey::Insert);
    }
}
