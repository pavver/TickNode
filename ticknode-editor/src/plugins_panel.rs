//! "Plugins" panel: plugin list with status, enable toggle, capabilities, and settings form
//! driven by the manifest schema. Secrets are never sent to the editor: password fields only show
//! "set / unset" and submit either a new value or "keep unchanged".

use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::{self, t};
use crate::state::{LogKind, use_editor};
use crate::ui::*;

/// Form field text for a stored setting value.
fn draft_of(view: &SettingValueView) -> String {
    match view {
        SettingValueView::Value(DataValue::Bool(b)) => b.to_string(),
        SettingValueView::Value(v) => text::format_value(v),
        SettingValueView::Secret { .. } | SettingValueView::Unset => String::new(),
    }
}

/// Update to send to the server for a field: `Err` if the text fails type validation.
fn update_of(plugin: &str, def: &SettingDefinition, draft: &str, cleared: bool) -> Result<SettingUpdate, String> {
    Ok(match &def.kind {
        SettingKind::Secret if cleared => SettingUpdate::Clear,
        SettingKind::Secret if draft.is_empty() => SettingUpdate::Keep,
        SettingKind::Bool => SettingUpdate::Set(DataValue::Bool(draft == "true")),
        SettingKind::Number if draft.trim().is_empty() => SettingUpdate::Clear,
        SettingKind::Number => SettingUpdate::Set(
            i18n::parse_value_localized(draft, &DataType::Number)
                .map_err(|e| t!("plugins-field-error", field = i18n::setting_title(plugin, def), error = e))?,
        ),
        _ if draft.is_empty() => SettingUpdate::Clear,
        _ => SettingUpdate::Set(DataValue::String(draft.to_string())),
    })
}

fn capability_text(c: &Capability) -> String {
    match c {
        Capability::Http { allowed_domains } if allowed_domains.iter().any(|d| d == "*") => t!("plugins-cap-http-any"),
        Capability::Http { allowed_domains } => t!("plugins-cap-http-domains", domains = allowed_domains.join(", ")),
        Capability::Storage => t!("plugins-cap-storage"),
        Capability::WebSocket { allowed_hosts } if allowed_hosts.iter().any(|d| d == "*") => t!("plugins-cap-ws-any"),
        Capability::WebSocket { allowed_hosts } => t!("plugins-cap-ws-hosts", hosts = allowed_hosts.join(", ")),
    }
}

#[component]
pub fn PluginsPanel() -> impl IntoView {
    let ed = use_editor();
    let names = Memo::new(move |_| ed.plugins.with(|p| p.iter().map(|i| i.name.clone()).collect::<Vec<_>>()));
    view! {
        <aside class="panel right plugins" class:hidden=move || !ed.show_plugins.get()>
            <div class="console-bar">
                <span class="t-section" style="flex:1">{move || t!("plugins-title")}</span>
                <IconButton icon=icons::CLOSE title=Signal::derive(move || t!("plugins-hide")) on_click=Callback::new(move |_| ed.show_plugins.set(false)) />
            </div>
            {move || names.with(Vec::is_empty).then(|| view! {
                <div class="empty">{move || t!("plugins-empty")}</div>
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

    // Server response (or error) resolves the pending state of the toggle and "Save" button
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

    // Form drafts: reset when the server confirms updated values
    let drafts = RwSignal::new(BTreeMap::<String, String>::new());
    let cleared = RwSignal::new(BTreeSet::<String>::new());
    let values = Memo::new(move |_| info.with(|i| i.as_ref().map(|i| i.values.clone()).unwrap_or_default()));
    Effect::new(move |_| {
        let values = values.get();
        drafts.set(values.iter().map(|(k, v)| (k.clone(), draft_of(v))).collect());
        cleared.set(BTreeSet::new());
    });

    let state = move || info.with(|i| i.as_ref().map_or(PluginState::Error, |i| i.state));
    let chip = move || state_chip(state());
    let settings_error = move || info.with(|i| i.as_ref().and_then(|i| i.settings_error.clone()));
    let load_error = move || info.with(|i| i.as_ref().and_then(|i| i.error.clone()));

    let toggle_name = name.clone();
    let on_toggle = Callback::new(move |enabled: bool| {
        toggling.set(ControlState::Pending);
        ed.send(ClientMessage::SetPluginEnabled { name: toggle_name.clone(), enabled });
    });

    let save_name = name.clone();
    let save_defs = initial.settings.clone();
    let save = Callback::new(move |()| {
        let updates = drafts.with_untracked(|drafts| cleared.with_untracked(|cleared| form_updates(&save_name, &save_defs, drafts, cleared)));
        let updates = match updates {
            Ok(updates) => updates,
            Err(error) => {
                ed.log(LogKind::Error, format!("[ERROR] {error}"));
                flash(saving, ControlState::Error);
                return;
            }
        };
        saving.set(ControlState::Pending);
        ed.send(ClientMessage::ConfigurePlugin { name: save_name.clone(), values: updates });
    });

    let fields = initial.settings.iter().cloned().map(|def| view! { <SettingField plugin=name.clone() def=def drafts=drafts cleared=cleared values=values /> }).collect_view();
    let has_settings = !initial.settings.is_empty();
    let caps = initial.capabilities.clone();

    view! {
        <section class="panel-section plugin-card">
            <div class="plugin-head">
                <div class="plugin-title">
                    <span class="t-row ellipsis">{initial.name.clone()}</span>
                    <span class="t-caption ellipsis">
                        {plugin_caption(&initial)}
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
                {move || settings_error().map(|_| view! { <span class="chip st-error" title=move || t!("plugins-settings-error-title")>{move || t!("plugins-settings-error-chip")}</span> })}
            </div>
            {move || load_error().map(|e| view! { <div class="plugin-error">{i18n::ui_text(&e)}</div> })}
            {(!caps.is_empty()).then(|| view! {
                <div class="caps">
                    <span class="field-label">{move || t!("plugins-capabilities")}</span>
                    {caps.into_iter().map(|c| view! { <span class="t-body-small">{move || capability_text(&c)}</span> }).collect_view()}
                </div>
            })}
            {has_settings.then(|| view! {
                <button class="section-toggle" on:click=move |_| expanded.update(|v| *v = !*v)>
                    <span>{move || if expanded.get() { "▾" } else { "▸" }}</span>
                    {move || t!("plugins-settings")}
                </button>
            })}
            {has_settings.then(|| view! {
                <div class="plugin-form" class:hidden=move || !expanded.get()>
                    {fields}
                    <ActionButton text=Signal::derive(move || t!("plugins-save")) icon=icons::CHECK full=true state=Signal::derive(move || saving.get()) on_click=save />
                    {move || settings_error().map(|e| view! { <div class="plugin-error">{i18n::ui_text(&e)}</div> })}
                </div>
            })}
        </section>
    }
    .into_any()
}

fn state_chip(state: PluginState) -> (ControlState, String) {
    match state {
        PluginState::Enabled => (ControlState::Success, t!("plugins-state-enabled")),
        PluginState::Disabled => (ControlState::Idle, t!("plugins-state-disabled")),
        PluginState::Error => (ControlState::Error, t!("plugins-state-error")),
    }
}

fn plugin_caption(info: &PluginInfo) -> String {
    let no_manifest = info.version.is_empty() && info.settings.is_empty() && info.capabilities.is_empty() && info.author.is_empty();
    if no_manifest { info.file.clone() } else { format!("v{} · {}", info.version, info.author) }
}

fn form_updates(plugin: &str, definitions: &[SettingDefinition], drafts: &BTreeMap<String, String>, cleared: &BTreeSet<String>) -> Result<Vec<(String, SettingUpdate)>, String> {
    definitions.iter().map(|def| {
        let draft = drafts.get(&def.key).map_or("", String::as_str);
        update_of(plugin, def, draft, cleared.contains(&def.key)).map(|update| (def.key.clone(), update))
    }).collect()
}

/// Single form field rendered according to the setting kind.
#[component]
fn SettingField(
    plugin: String,
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
            let option_plugin = plugin.clone();
            let option_def = def.clone();
            view! {
                <select class="form-input" prop:value=move || draft.get() on:change=move |ev| set_draft(event_target_value(&ev))>
                    {unset.then(|| view! { <option value="">"—"</option> })}
                    {options.into_iter().map(|(value, title)| {
                        let (plugin, def) = (option_plugin.clone(), option_def.clone());
                        let label = { let value = value.clone(); move || i18n::setting_option_label(&plugin, &def, &value, &title) };
                        view! { <option value=value.clone()>{label}</option> }
                    }).collect_view()}
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
                        placeholder=move || if is_cleared.get() { t!("plugins-secret-will-clear") } else if is_set.get() { t!("plugins-secret-set") } else { t!("plugins-secret-unset") }
                        prop:value=move || draft.get()
                        on:input=move |ev| set_draft(event_target_value(&ev))
                    />
                    {move || (is_set.get() && !is_cleared.get()).then(|| {
                        let clear_key = clear_key.clone();
                        view! {
                            <button class="link-btn" title=move || t!("plugins-secret-clear-title") on:click=move |_| {
                                cleared.update(|c| { c.insert(clear_key.clone()); });
                                drafts.update(|d| { d.insert(clear_key.clone(), String::new()); });
                            }>{move || t!("plugins-secret-clear")}</button>
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
    let (title_plugin, title_def) = (plugin.clone(), def.clone());
    let (desc_plugin, desc_def) = (plugin, def.clone());
    view! {
        <div class="field">
            <label class="field-label">{move || i18n::setting_title(&title_plugin, &title_def)}{def.required.then_some(" *")}</label>
            {control}
            {move || Some(i18n::setting_description(&desc_plugin, &desc_def)).filter(|d| !d.is_empty()).map(|d| view! { <span class="t-caption">{d}</span> })}
        </div>
    }
}

#[cfg(test)]
mod tests;
