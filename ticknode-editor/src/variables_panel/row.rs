//! Variable row component with in-place value editing, drag to canvas, action button, and delete confirmation.

use super::logic::VariableItem;
use super::menu::handle_variable_click;
use super::value_editor::VariableValueEditor;
use crate::geometry::type_color;
use crate::i18n::{self, t};
use crate::panels::{drop_on_canvas, move_palette_drag};
use crate::state::{PaletteDrag, use_editor};
use leptos::prelude::*;
use ticknode_shared::ClientMessage;

#[component]
pub fn VariableRow(item: VariableItem, #[prop(optional)] can_drag: bool) -> impl IntoView {
    let ed = use_editor();
    let key = item.variable.key.clone();
    let key_for_var = key.clone();
    let initial_var = item.variable.clone();

    // Reactively track the variable from state so server updates reflect here
    let var_signal = Memo::new(move |_| {
        ed.variables
            .with(|vars| vars.get(&key_for_var).cloned())
            .unwrap_or_else(|| initial_var.clone())
    });

    let confirming_delete = RwSignal::new(false);
    let stop = |ev: web_sys::PointerEvent| ev.stop_propagation();

    let start_drag = move |ev: web_sys::PointerEvent| {
        if !can_drag || ev.button() != 0 {
            return;
        }
        use wasm_bindgen::JsCast;
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        let at = (ev.client_x() as f64, ev.client_y() as f64);
        let cur_var = var_signal.get_untracked();
        let color = ed
            .types
            .with_untracked(|types| type_color(types, &cur_var.data_type));
        ed.palette_drag.set(Some(PaletteDrag::Variable {
            variable: cur_var.clone(),
            title: cur_var.key.name.clone(),
            color,
            start: at,
            at,
            active: false,
        }));
    };

    let on_pointer_move = move |ev: web_sys::PointerEvent| {
        if can_drag {
            move_palette_drag(ed, &ev);
        }
    };

    let on_pointer_up = move |ev: web_sys::PointerEvent| {
        if can_drag {
            match ed.palette_drag.try_update(Option::take).flatten() {
                Some(drag) if drag.active() => drop_on_canvas(ed, &drag, &ev),
                _ => {}
            }
        }
    };

    let on_pointer_cancel = move |_| {
        if can_drag {
            ed.palette_drag.set(None);
        }
    };

    let var_name = item.variable.key.name.clone();
    let is_shadowed = item.shadowed;
    let key_for_commit = key.clone();
    let key_for_delete = key.clone();

    view! {
        <div
            class=move || {
                let mut c = "var-row".to_string();
                if is_shadowed {
                    c.push_str(" shadowed");
                }
                if can_drag {
                    c.push_str(" draggable");
                }
                c
            }
            on:pointerdown=start_drag
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up
            on:pointercancel=on_pointer_cancel
        >
            <div class="var-row-header">
                <div class="var-row-title">
                    <span class="var-name" title=var_name.clone()>{var_name.clone()}</span>
                    {is_shadowed.then(|| view! {
                        <span class="var-shadowed" title=move || t!("variables-shadowed")>
                            {move || t!("variables-shadowed")}
                        </span>
                    })}
                </div>
                <div class="var-row-meta">
                    {move || {
                        let cur_var = var_signal.get();
                        let title = ed.types.with(|types| i18n::data_type_title(types, &cur_var.data_type));
                        let color = ed.types.with(|types| type_color(types, &cur_var.data_type));
                        view! {
                            <span class="var-type-chip" style=format!("--chip-color:{color}")>
                                {title}
                            </span>
                        }
                    }}
                    <div class="var-row-actions">
                        {can_drag.then(|| {
                            let var_for_btn = var_signal;
                            view! {
                                <button
                                    class="icon-btn btn-xs var-add-btn"
                                    title=move || t!("variables-add-to-canvas")
                                    on:pointerdown=stop
                                    on:click=move |ev: web_sys::MouseEvent| {
                                        let screen = (ev.client_x() as f64, ev.client_y() as f64);
                                        handle_variable_click(ed, &var_for_btn.get_untracked(), screen);
                                    }
                                >
                                    <span class="mdi mdi-plus-box-outline" aria-hidden="true"></span>
                                </button>
                            }
                        })}
                        <button
                            class="icon-btn btn-xs var-delete-btn"
                            title=move || t!("variables-action-delete")
                            on:pointerdown=stop
                            on:click=move |_| confirming_delete.set(true)
                        >
                            <span class="mdi mdi-delete-outline" aria-hidden="true"></span>
                        </button>
                    </div>
                </div>
            </div>

            <div class="var-row-value">
                {move || {
                    let cur_var = var_signal.get();
                    let key_c = key_for_commit.clone();
                    view! {
                        <VariableValueEditor
                            data_type=cur_var.data_type.clone()
                            value=Signal::derive(move || cur_var.value.clone())
                            on_commit=Callback::new(move |new_val| {
                                ed.send(ClientMessage::SetVariable {
                                    key: key_c.clone(),
                                    value: new_val,
                                });
                            })
                        />
                    }
                }}
            </div>

            {move || confirming_delete.get().then(|| {
                let name_for_del = var_name.clone();
                let key_del = key_for_delete.clone();
                view! {
                    <div class="var-delete-confirm" on:pointerdown=stop>
                        <span class="var-delete-text">
                            {move || t!("variables-delete-confirm", name = name_for_del.clone())}
                        </span>
                        <div class="var-delete-actions">
                            <button
                                class="btn danger ctl btn-sm"
                                on:click=move |_| {
                                    ed.send(ClientMessage::DeleteVariable(key_del.clone()));
                                    confirming_delete.set(false);
                                }
                            >
                                <span>{move || t!("variables-action-delete")}</span>
                            </button>
                            <button
                                class="btn ctl btn-sm"
                                on:click=move |_| confirming_delete.set(false)
                            >
                                <span>{move || t!("variables-cancel")}</span>
                            </button>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
