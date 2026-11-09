//! Settings page: interface preferences and engine configuration.

pub mod logic;

use leptos::prelude::*;
use ticknode_shared::ClientMessage;
use crate::i18n::{t, Locale};
use crate::node_status;
use crate::state::{use_editor, Theme};
use crate::ui::{icons, ActionButton, ControlState, Switch};
use logic::EngineSettingsDraft;

#[cfg(test)]
mod tests;

#[component]
pub fn SettingsSection() -> impl IntoView {
    view! {
        <div class="home-settings-section">
            <InterfaceGroup />
            <EngineGroup />
        </div>
    }
}

#[component]
fn InterfaceGroup() -> impl IntoView {
    let ed = use_editor();

    view! {
        <section class="settings-group">
            <div class="settings-group-header">
                <span class="t-section">{move || t!("settings-interface")}</span>
            </div>
            <div class="settings-row">
                <div class="settings-row-head">
                    <span class="settings-label">{move || t!("settings-language")}</span>
                    <select
                        class="form-input settings-control"
                        prop:value=move || ed.locale.get().code()
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            if let Some(loc) = Locale::from_tag(&val) {
                                ed.locale.set(loc);
                            }
                        }
                    >
                        <option value="en" selected=move || ed.locale.get() == Locale::En>"English"</option>
                        <option value="uk" selected=move || ed.locale.get() == Locale::Uk>"Українська"</option>
                    </select>
                </div>
                <span class="settings-description">{move || t!("settings-language-description")}</span>
            </div>
            <div class="settings-row">
                <div class="settings-row-head">
                    <span class="settings-label">{move || t!("settings-theme")}</span>
                    <select
                        class="form-input settings-control"
                        prop:value=move || match ed.theme.get() { Theme::Dark => "dark", Theme::Light => "light" }
                        on:change=move |ev| {
                            match event_target_value(&ev).as_str() {
                                "light" => ed.theme.set(Theme::Light),
                                _ => ed.theme.set(Theme::Dark),
                            }
                        }
                    >
                        <option value="dark" selected=move || ed.theme.get() == Theme::Dark>{move || t!("settings-theme-dark")}</option>
                        <option value="light" selected=move || ed.theme.get() == Theme::Light>{move || t!("settings-theme-light")}</option>
                    </select>
                </div>
                <span class="settings-description">{move || t!("settings-theme-description")}</span>
            </div>
            <div class="settings-row">
                <div class="settings-row-head">
                    <span class="settings-label">{move || t!("settings-show-values")}</span>
                    <Switch
                        checked=Signal::derive(move || ed.show_values.get())
                        on_toggle=Callback::new(move |checked| node_status::set_show_values(ed, checked))
                    />
                </div>
                <span class="settings-description">{move || t!("settings-show-values-description")}</span>
            </div>
        </section>
    }
}

#[component]
fn EngineGroup() -> impl IntoView {
    let ed = use_editor();
    let draft = RwSignal::new(EngineSettingsDraft::new(ed.server_settings.get_untracked()));
    let saving = RwSignal::new(ControlState::Idle);

    Effect::new(move |_| {
        let current = ed.server_settings.get();
        if !draft.with_untracked(|d| d.is_dirty(current)) {
            draft.set(EngineSettingsDraft::new(current));
        }
    });

    Effect::new(move |_| {
        ed.server_settings.track();
        if saving.get_untracked() == ControlState::Pending {
            crate::ui::flash(saving, ControlState::Success);
        }
    });

    Effect::new(move |_| {
        ed.error_seq.track();
        if saving.get_untracked() == ControlState::Pending {
            crate::ui::flash(saving, ControlState::Error);
        }
    });

    let can_save = Signal::derive(move || draft.with(|d| d.can_save(ed.server_settings.get())));
    let on_save = Callback::new(move |()| {
        if let Some(settings) = draft.with_untracked(|d| d.to_server_settings()) {
            saving.set(ControlState::Pending);
            ed.send(ClientMessage::SetServerSettings(settings));
        }
    });
    let on_reset = move |_| {
        draft.set(EngineSettingsDraft::new(ed.server_settings.get_untracked()));
    };

    view! {
        <section class="settings-group">
            <div class="settings-group-header">
                <span class="t-section">{move || t!("settings-engine")}</span>
                <span class="settings-description">{move || t!("settings-engine-note")}</span>
            </div>
            <div class="settings-row">
                <div class="settings-row-head">
                    <span class="settings-label">{move || t!("settings-tick")}</span>
                    <input
                        type="text"
                        inputmode="numeric"
                        class="form-input settings-control"
                        class:st-error=move || !draft.with(|d| d.is_tick_valid())
                        prop:value=move || draft.with(|d| d.tick_ms.clone())
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            draft.update(|d| d.tick_ms = val);
                        }
                    />
                </div>
                {move || (!draft.with(|d| d.is_tick_valid())).then(|| view! {
                    <div class="settings-error">{move || t!("settings-invalid-number")}</div>
                })}
                <span class="settings-description">{move || t!("settings-tick-description")}</span>
            </div>
            <div class="settings-row">
                <div class="settings-row-head">
                    <span class="settings-label">{move || t!("settings-budget")}</span>
                    <input
                        type="text"
                        inputmode="numeric"
                        class="form-input settings-control"
                        class:st-error=move || !draft.with(|d| d.is_budget_valid())
                        prop:value=move || draft.with(|d| d.tick_budget.clone())
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            draft.update(|d| d.tick_budget = val);
                        }
                    />
                </div>
                {move || (!draft.with(|d| d.is_budget_valid())).then(|| view! {
                    <div class="settings-error">{move || t!("settings-invalid-number")}</div>
                })}
                <span class="settings-description">{move || t!("settings-budget-description")}</span>
            </div>
            <div class="settings-actions">
                <ActionButton
                    text=Signal::derive(move || t!("settings-save"))
                    icon=icons::CHECK
                    disabled=Signal::derive(move || !can_save.get())
                    state=Signal::derive(move || saving.get())
                    on_click=on_save
                />
                <button
                    class="btn ctl"
                    disabled=move || !draft.with(|d| d.is_dirty(ed.server_settings.get()))
                    on:click=on_reset
                >
                    {move || t!("settings-reset")}
                </button>
            </div>
        </section>
    }
}
