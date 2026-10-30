//! Панель «Плагіни»: список плагінів зі станом, перемикачем увімкнення, правами та формою
//! налаштувань за схемою з маніфесту. Секрети не надходять у редактор: поле пароля показує лише
//! «задано / не задано» і надсилає або нове значення, або «не змінювати».

use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use ticknode_shared::*;

use crate::state::{LogKind, use_editor};
use crate::ui::*;

/// Текст у полі форми для збереженого значення.
fn draft_of(view: &SettingValueView) -> String {
    match view {
        SettingValueView::Value(DataValue::Bool(b)) => b.to_string(),
        SettingValueView::Value(v) => text::format_value(v),
        SettingValueView::Secret { .. } | SettingValueView::Unset => String::new(),
    }
}

/// Що надіслати серверу для поля: `Err` — текст не підходить за типом.
fn update_of(def: &SettingDefinition, draft: &str, cleared: bool) -> Result<SettingUpdate, String> {
    Ok(match &def.kind {
        SettingKind::Secret if cleared => SettingUpdate::Clear,
        SettingKind::Secret if draft.is_empty() => SettingUpdate::Keep,
        SettingKind::Bool => SettingUpdate::Set(DataValue::Bool(draft == "true")),
        SettingKind::Number if draft.trim().is_empty() => SettingUpdate::Clear,
        SettingKind::Number => SettingUpdate::Set(
            text::parse_value(draft, &DataType::Number).map_err(|e| format!("«{}»: {e}", def.title))?,
        ),
        _ if draft.is_empty() => SettingUpdate::Clear,
        _ => SettingUpdate::Set(DataValue::String(draft.to_string())),
    })
}

fn capability_text(c: &Capability) -> String {
    match c {
        Capability::Http { allowed_domains } if allowed_domains.iter().any(|d| d == "*") => "HTTP: будь-які домени".to_string(),
        Capability::Http { allowed_domains } => format!("HTTP: {}", allowed_domains.join(", ")),
        Capability::Storage => "Сховище: власний каталог плагіна".to_string(),
    }
}

#[component]
pub fn PluginsPanel() -> impl IntoView {
    let ed = use_editor();
    let names = Memo::new(move |_| ed.plugins.with(|p| p.iter().map(|i| i.name.clone()).collect::<Vec<_>>()));
    view! {
        <aside class="panel right plugins" class:hidden=move || !ed.show_plugins.get()>
            <div class="console-bar">
                <span class="t-section" style="flex:1">"Плагіни"</span>
                <IconButton icon=icons::CLOSE title="Сховати панель плагінів" on_click=Callback::new(move |_| ed.show_plugins.set(false)) />
            </div>
            {move || names.with(Vec::is_empty).then(|| view! {
                <div class="empty">"Плагінів не знайдено. Покладіть скомпільовані компоненти (*.wasm) у data/plugins/ і перезапустіть сервер."</div>
            })}
            <For each=move || names.get() key=|n| n.clone() let:name>
                <PluginCard name=name />
            </For>
        </aside>
    }
}

#[component]
fn PluginCard(name: String) -> impl IntoView {
    let ed = use_editor();
    let lookup = name.clone();
    let info = Memo::new(move |_| ed.plugins.with(|p| p.iter().find(|i| i.name == lookup).cloned()));
    let Some(initial) = info.get_untracked() else { return ().into_any() };

    let expanded = RwSignal::new(false);
    let toggling = RwSignal::new(ControlState::Idle);
    let saving = RwSignal::new(ControlState::Idle);

    // Відповідь сервера (або помилка) завершує очікування перемикача й кнопки «Зберегти»
    Effect::new(move |_| {
        ed.plugins.track();
        let settings_failed = info.with_untracked(|i| i.as_ref().is_some_and(|i| i.settings_error.is_some()));
        if toggling.get_untracked() == ControlState::Pending {
            toggling.set(ControlState::Idle);
        }
        if saving.get_untracked() == ControlState::Pending {
            flash(saving, if settings_failed { ControlState::Error } else { ControlState::Success });
        }
    });
    Effect::new(move |_| {
        ed.error_seq.track();
        if toggling.get_untracked() == ControlState::Pending {
            flash(toggling, ControlState::Error);
        }
        if saving.get_untracked() == ControlState::Pending {
            flash(saving, ControlState::Error);
        }
    });

    // Чернетки форми: скидаються, коли сервер підтвердив нові значення
    let drafts = RwSignal::new(BTreeMap::<String, String>::new());
    let cleared = RwSignal::new(BTreeSet::<String>::new());
    let values = Memo::new(move |_| info.with(|i| i.as_ref().map(|i| i.values.clone()).unwrap_or_default()));
    Effect::new(move |_| {
        let values = values.get();
        drafts.set(values.iter().map(|(k, v)| (k.clone(), draft_of(v))).collect());
        cleared.set(BTreeSet::new());
    });

    let state = move || info.with(|i| i.as_ref().map_or(PluginState::Error, |i| i.state));
    let chip = move || match state() {
        PluginState::Enabled => (ControlState::Success, "працює"),
        PluginState::Disabled => (ControlState::Idle, "вимкнено"),
        PluginState::Error => (ControlState::Error, "помилка"),
    };
    let settings_error = move || info.with(|i| i.as_ref().and_then(|i| i.settings_error.clone()));
    let load_error = move || info.with(|i| i.as_ref().and_then(|i| i.error.clone()));
    let no_manifest = initial.version.is_empty() && initial.settings.is_empty() && initial.capabilities.is_empty() && initial.author.is_empty();

    let toggle_name = name.clone();
    let on_toggle = Callback::new(move |enabled: bool| {
        toggling.set(ControlState::Pending);
        ed.send(ClientMessage::SetPluginEnabled { name: toggle_name.clone(), enabled });
    });

    let save_name = name.clone();
    let save_defs = initial.settings.clone();
    let save = Callback::new(move |()| {
        let mut updates = Vec::new();
        for def in &save_defs {
            let draft = drafts.with_untracked(|d| d.get(&def.key).cloned().unwrap_or_default());
            let is_cleared = cleared.with_untracked(|c| c.contains(&def.key));
            match update_of(def, &draft, is_cleared) {
                Ok(update) => updates.push((def.key.clone(), update)),
                Err(e) => {
                    ed.log(LogKind::Error, format!("[ERROR] {e}"));
                    flash(saving, ControlState::Error);
                    return;
                }
            }
        }
        saving.set(ControlState::Pending);
        ed.send(ClientMessage::ConfigurePlugin { name: save_name.clone(), values: updates });
    });

    let fields = initial.settings.iter().cloned().map(|def| view! { <SettingField def=def drafts=drafts cleared=cleared values=values /> }).collect_view();
    let has_settings = !initial.settings.is_empty();
    let caps = initial.capabilities.iter().map(capability_text).collect::<Vec<_>>();

    view! {
        <section class="panel-section plugin-card">
            <div class="plugin-head">
                <div class="plugin-title">
                    <span class="t-row ellipsis">{initial.name.clone()}</span>
                    <span class="t-caption ellipsis">
                        {if no_manifest { initial.file.clone() } else { format!("v{} · {}", initial.version, initial.author) }}
                    </span>
                </div>
                <Switch
                    checked=Signal::derive(move || state() == PluginState::Enabled)
                    on_toggle=on_toggle
                    state=Signal::derive(move || toggling.get())
                />
            </div>
            <div class="chips">
                <span class=move || format!("chip {}", chip().0.class())>{move || chip().1}</span>
                {move || settings_error().map(|_| view! { <span class="chip st-error" title="Плагін відхилив налаштування">"помилка налаштувань"</span> })}
            </div>
            {move || load_error().map(|e| view! { <div class="plugin-error">{e}</div> })}
            {(!caps.is_empty()).then(|| view! {
                <div class="caps">
                    <span class="field-label">"Права"</span>
                    {caps.iter().map(|c| view! { <span class="t-body-small">{c.clone()}</span> }).collect_view()}
                </div>
            })}
            {has_settings.then(|| view! {
                <button class="section-toggle" on:click=move |_| expanded.update(|v| *v = !*v)>
                    <span>{move || if expanded.get() { "▾" } else { "▸" }}</span>
                    "Налаштування"
                </button>
            })}
            {has_settings.then(|| view! {
                <div class="plugin-form" class:hidden=move || !expanded.get()>
                    {fields}
                    <ActionButton text="Зберегти" icon=icons::CHECK full=true state=Signal::derive(move || saving.get()) on_click=save />
                    {move || settings_error().map(|e| view! { <div class="plugin-error">{e}</div> })}
                </div>
            })}
        </section>
    }
    .into_any()
}

/// Одне поле форми за типом налаштування.
#[component]
fn SettingField(
    def: SettingDefinition,
    drafts: RwSignal<BTreeMap<String, String>>,
    cleared: RwSignal<BTreeSet<String>>,
    values: Memo<Vec<(String, SettingValueView)>>,
) -> impl IntoView {
    let key = def.key.clone();
    let draft = {
        let key = key.clone();
        Signal::derive(move || drafts.with(|d| d.get(&key).cloned().unwrap_or_default()))
    };
    let set_draft = {
        let key = key.clone();
        move |v: String| drafts.update(|d| { d.insert(key.clone(), v); })
    };
    let control = match def.kind.clone() {
        SettingKind::Bool => view! {
            <Switch checked=Signal::derive(move || draft.get() == "true") on_toggle=Callback::new(move |b: bool| set_draft(b.to_string())) />
        }
        .into_any(),
        SettingKind::Select(options) => {
            let unset = draft.get_untracked().is_empty();
            view! {
                <select class="form-input" prop:value=move || draft.get() on:change=move |ev| set_draft(event_target_value(&ev))>
                    {unset.then(|| view! { <option value="">"—"</option> })}
                    {options.into_iter().map(|(value, title)| view! { <option value=value.clone()>{title}</option> }).collect_view()}
                </select>
            }
            .into_any()
        }
        SettingKind::Secret => {
            let secret_key = key.clone();
            let is_set = Signal::derive(move || values.with(|v| v.iter().any(|(k, s)| *k == secret_key && matches!(s, SettingValueView::Secret { is_set: true }))));
            let clear_key = key.clone();
            let cleared_key = key.clone();
            let is_cleared = Signal::derive(move || cleared.with(|c| c.contains(&cleared_key)));
            view! {
                <div class="secret-row">
                    <input
                        class="form-input"
                        type="password"
                        autocomplete="new-password"
                        spellcheck="false"
                        placeholder=move || if is_cleared.get() { "буде очищено" } else if is_set.get() { "задано" } else { "не задано" }
                        prop:value=move || draft.get()
                        on:input=move |ev| set_draft(event_target_value(&ev))
                    />
                    {move || (is_set.get() && !is_cleared.get()).then(|| {
                        let clear_key = clear_key.clone();
                        view! {
                            <button class="link-btn" title="Видалити збережене значення" on:click=move |_| {
                                cleared.update(|c| { c.insert(clear_key.clone()); });
                                drafts.update(|d| { d.insert(clear_key.clone(), String::new()); });
                            }>"Очистити"</button>
                        }
                    })}
                </div>
            }
            .into_any()
        }
        kind => view! {
            <input
                class="form-input"
                type="text"
                inputmode=if kind == SettingKind::Number { "decimal" } else { "text" }
                spellcheck="false"
                placeholder=if kind == SettingKind::Url { "https://" } else { "" }
                prop:value=move || draft.get()
                on:input=move |ev| set_draft(event_target_value(&ev))
            />
        }
        .into_any(),
    };
    view! {
        <div class="field">
            <label class="field-label">{def.title.clone()}{def.required.then_some(" *")}</label>
            {control}
            {(!def.description.is_empty()).then(|| view! { <span class="t-caption">{def.description.clone()}</span> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_values_map_to_updates() {
        let text = SettingDefinition::text("greeting", "Привітання");
        let secret = SettingDefinition::secret("token", "Токен");
        let number = SettingDefinition::number("port", "Порт");
        let flag = SettingDefinition::bool("tls", "TLS");
        assert_eq!(update_of(&text, "hi", false), Ok(SettingUpdate::Set(DataValue::String("hi".into()))));
        assert_eq!(update_of(&text, "", false), Ok(SettingUpdate::Clear));
        // Секрет: порожнє поле — «не змінювати», явне очищення — Clear
        assert_eq!(update_of(&secret, "", false), Ok(SettingUpdate::Keep));
        assert_eq!(update_of(&secret, "", true), Ok(SettingUpdate::Clear));
        assert_eq!(update_of(&secret, "abc", false), Ok(SettingUpdate::Set(DataValue::String("abc".into()))));
        assert_eq!(update_of(&number, "8123", false), Ok(SettingUpdate::Set(DataValue::Number(NumberValue::Int(8123)))));
        assert!(update_of(&number, "abc", false).is_err());
        assert_eq!(update_of(&number, " ", false), Ok(SettingUpdate::Clear));
        assert_eq!(update_of(&flag, "true", false), Ok(SettingUpdate::Set(DataValue::Bool(true))));
        assert_eq!(update_of(&flag, "", false), Ok(SettingUpdate::Set(DataValue::Bool(false))));
    }

    #[test]
    fn secrets_never_prefill_the_form() {
        assert_eq!(draft_of(&SettingValueView::Secret { is_set: true }), "");
        assert_eq!(draft_of(&SettingValueView::Value(DataValue::Bool(true))), "true");
        assert_eq!(draft_of(&SettingValueView::Value(DataValue::String("x".into()))), "x");
    }
}
