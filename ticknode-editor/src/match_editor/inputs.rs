//! Match input rows and their name field.

use super::*;
use crate::canvas::PinSplitControl;
use crate::geometry::type_color;
use crate::inline::PinValue;
use crate::state::LogKind;
use crate::ui::{ControlState, flash};
use super::editing::{validate_name, rename_input, set_input_type, remove_input};

#[component]
pub(super) fn InputRow(node: NodeId, state: Memo<MatchState>, types: Memo<InputTypes>, ii: usize, pin: u32, edit: Callback<EditFn>) -> impl IntoView {
    let ed = use_editor();
    let pin_id = PinId(pin);
    let input = Memo::new(move |_| state.with(|s| s.inputs.get(ii).cloned()));
    let name = Signal::derive(move || input.with(|i| i.as_ref().map(|i| i.name.clone()).unwrap_or_default()));
    let declared = Memo::new(move |_| input.with(|i| i.as_ref().map_or(DataType::Any, |i| i.value_type.clone())));
    let effective = Memo::new(move |_| types.with(|t| t.get(ii).map_or(DataType::Any, |(_, t)| t.clone())));
    let connected = Memo::new(move |_| ed.is_input_connected(node, pin_id));

    let rename = Callback::new(move |new: String| -> Result<(), String> {
        let new = state.with_untracked(|s| validate_name(s, ii, &new))?;
        edit.run(EditFn::new(move |s, _| rename_input(s, ii, new)));
        Ok(())
    });
    let set_type = move |t: DataType| edit.run(EditFn::new(move |s, env| set_input_type(s, env, ii, t)));
    let remove = move |_| edit.run(EditFn::new(move |s, env| remove_input(s, env, ii)));
    let pin_title = move || format!("{}: {}", name.get(), ed.type_title(&effective.get()));
    view! {
        <div class="case-row input-row">
            {move || pin_def(node, pin_id, PinDirection::Input).map(|_| view! {
                <div class="pin in match-socket" title=pin_title data-pin=pin.to_string() data-node=node.0.to_string() data-dir="in" data-kind="data">
                    <span class="socket" class:connected=move || connected.get() style=move || format!("--pin-color:{}", ed.types.with(|r| type_color(r, &effective.get())))></span>
                </div>
            })}
            {move || pin_def(node, pin_id, PinDirection::Input).map(|p| view! { <PinSplitControl node=node pin=p /> })}
            <TextField value=name on_commit=rename class="input-name" title=Signal::derive(move || t!("match-input-name-title")) />
            <select
                class="op-select type-select"
                title=move || t!("match-input-type-title")
                on:change=move |ev| {
                    if let Some(t) = parse_input_type(&event_target_value(&ev)) {
                        set_type(t);
                    }
                }
            >
                {move || INPUT_TYPES.iter().map(|t| (t.to_string(), if *t == "any" { i18n::type_name_label("auto") } else { i18n::type_name_label(t) }))
                    .chain(ed.types.with(|r| r.definitions()).into_iter().map(|d| (d.name.clone(), i18n::type_def_title(&d))))
                    .map(|(t, label)| {
                        let value = t.clone();
                        view! { <option value=value selected=move || input_type_name(&declared.get()) == t>{label}</option> }
                    }).collect_view()}
            </select>
            {move || {
                let t = declared.get();
                if connected.get() {
                    view! { <span class="input-src ellipsis">{move || format!("← {}", ed.type_title(&effective.get()))}</span> }.into_any()
                } else if ed.types.with(|r| input_value_editable(r, &t)) {
                    view! { <PinValue ed=ed node=node pin=pin_id value_type=t /> }.into_any()
                } else {
                    view! { <span class="input-src ellipsis">{move || t!("match-connect-hint")}</span> }.into_any()
                }
            }}
            <button class="mini-btn icon danger" title=move || t!("match-remove-input-title") on:click=remove>
                <Icon path=icons::CLOSE />
            </button>
        </div>
        {move || ed.node_tracked(node).map(|n| n.instance.with(|i| i.definition.pins.iter()
            .filter(|p| p.direction == PinDirection::Input && split_field_parent(p.id).is_some_and(|(parent, _)| parent == pin_id))
            .cloned().collect::<Vec<_>>())).unwrap_or_default().into_iter().map(|p| view! {
                <div class="case-row split-input-row"><Pin node=node pin=p dir=PinDirection::Input /></div>
            }).collect_view()}
    }
}

/// Text input that commits on Enter or blur (Esc cancels). If `on_commit` returns an error,
/// reverts to the previous text and logs the error to the console.
#[component]
fn TextField(
    #[prop(into)] value: Signal<String>,
    on_commit: Callback<String, Result<(), String>>,
    #[prop(into)] class: String,
    #[prop(optional, into)] title: Signal<String>,
) -> impl IntoView {
    let ed = use_editor();
    let state = RwSignal::new(ControlState::Idle);
    let draft = RwSignal::new(String::new());
    Effect::new(move |_| draft.set(value.get()));
    let commit = move || {
        let (Some(current), Some(saved)) = (draft.try_get_untracked(), value.try_get_untracked()) else { return };
        if current.trim() == saved {
            return;
        }
        if let Err(e) = on_commit.run(current) {
            draft.set(saved);
            flash(state, ControlState::Error);
            ed.log(LogKind::Error, format!("[ERROR] {e}"));
        }
    };
    view! {
        <input
            type="text"
            class=move || format!("operand {class} {}", state.get().class())
            spellcheck="false"
            title=move || title.get()
            prop:value=move || draft.get()
            on:input=move |ev| draft.set(event_target_value(&ev))
            on:blur=move |_| commit()
            on:keydown=move |ev| {
                let input = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok());
                match ev.key().as_str() {
                    // Commits on blur
                    "Enter" => {
                        if let Some(i) = input { let _ = i.blur(); }
                    }
                    "Escape" => {
                        if let Some(saved) = value.try_get_untracked() { draft.set(saved); }
                        if let Some(i) = input { let _ = i.blur(); }
                    }
                    _ => {}
                }
            }
        />
    }
}
