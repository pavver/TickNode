//! Project units editor, sharing the engine settings Save/Reset draft.
pub mod logic;
use super::logic::EngineSettingsDraft;
use crate::i18n::{self, t};
use leptos::prelude::*;
use ticknode_shared::quantity;

#[component]
pub fn DisplayGroup(draft: RwSignal<EngineSettingsDraft>) -> impl IntoView {
    view! {
        <div class="settings-group-header"><span class="t-section">{move || t!("settings-display")}</span></div>
        <p class="settings-description">{move || t!("settings-display-description")}</p>
        {quantity::catalog().iter().map(|definition| {
            let name = definition.name.clone();
            view! { <QuantityRow name=name draft=draft /> }
        }).collect_view()}
        {move || draft.with(|d| d.display.parse().is_none()).then(|| view! {
            <div class="settings-error">{move || t!("settings-display-invalid")}</div>
        })}
    }
}

#[component]
fn QuantityRow(name: String, draft: RwSignal<EngineSettingsDraft>) -> impl IntoView {
    let name = StoredValue::new(name);
    let effective = move || draft.with(|d| d.display.effective(&name.get_value()));
    let q = quantity::kind(&name.get_value())
        .expect("catalog quantity")
        .clone();
    let units = StoredValue::new(q.units);
    let edit = move |f: Box<dyn FnOnce(&mut logic::RuleDraft, &quantity::QuantityKind)>| {
        draft.update(|d| d.display.edit(&name.get_value(), f));
    };
    view! {
        <div class="settings-row">
            <div class="settings-row-head">
                <span class="settings-label">{move || i18n::type_def_title(quantity::catalog().iter().find(|d| d.name == name.get_value()).expect("catalog quantity"))}</span>
                <button class="btn ctl" disabled=move || !draft.with(|d| d.display.rules.contains_key(&name.get_value()))
                    on:click=move |_| draft.update(|d| { d.display.rules.remove(&name.get_value()); })>
                    {move || t!("settings-display-default")}
                </button>
            </div>
            <For each={move || (0..effective().steps.len()).collect::<Vec<_>>()} key=|i| *i let:index>
                <div class="settings-row-head">
                    <label class="settings-description">{move || t!("settings-display-from")}</label>
                    <input class="form-input settings-control" type="text" inputmode="decimal"
                        aria-label=move || t!("settings-display-from") disabled=move || index == 0
                        prop:value=move || effective().steps.get(index).map(|s| s.threshold.clone()).unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            edit(Box::new(move |r, _| { if let Some(s) = r.steps.get_mut(index) { s.threshold = value; } }));
                        } />
                    <select class="form-input settings-control" aria-label=move || t!("quantity-unit-title")
                        prop:value=move || effective().steps.get(index).map(|s| s.unit.clone()).unwrap_or_default()
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            edit(Box::new(move |r, q| r.change_unit(index, value, q)));
                        }>
                        {units.get_value().into_iter().map(|u| {
                            let id = u.id.clone();
                            view! { <option value=u.id>{move || i18n::unit_symbol(&id).unwrap_or_else(|| u.symbol.clone())}</option> }
                        }).collect_view()}
                    </select>
                    <button class="btn ctl" aria-label=move || t!("settings-display-remove")
                        disabled=move || index == 0
                        on:click=move |_| edit(Box::new(move |r, _| { if index > 0 && index < r.steps.len() { r.steps.remove(index); } }))>
                        {move || t!("settings-display-remove")}
                    </button>
                </div>
            </For>
            <div class="settings-row-head">
                <button class="btn ctl" disabled={move || effective().steps.len() >= 8}
                    on:click=move |_| edit(Box::new(|r, q| r.add_step(q)))>{move || t!("settings-display-add")}</button>
                <label class="settings-description">{move || t!("settings-display-digits")}</label>
                <input class="form-input settings-control" type="text" inputmode="numeric"
                    aria-label=move || t!("settings-display-digits") prop:value=move || effective().digits
                    on:input=move |ev| {
                        let value = event_target_value(&ev);
                        edit(Box::new(move |r, _| r.digits = value));
                    } />
            </div>
        </div>
    }
}
