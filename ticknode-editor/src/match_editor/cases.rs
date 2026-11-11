//! Match case headers and condition rows.

use super::*;
use crate::geometry::type_color;
use crate::inline::InlineValue;

#[component]
pub(super) fn CaseBlock(node: NodeId, state: Memo<MatchState>, types: Memo<InputTypes>, ci: usize, pin: u32, edit: Callback<EditFn>) -> impl IntoView {
    let case = Memo::new(move |_| state.with(|s| s.cases.get(ci).cloned()));
    let rows = move || case.with(|c| c.as_ref().map_or(0, |c| c.conditions.len()));
    let out = Memo::new(move |_| pin_def(node, PinId(pin), PinDirection::Output));
    let count = move || state.with(|s| s.cases.len());
    view! {
        <div class="case-row case-head">
            <span class="case-index">{ci + 1}</span>
            <button class="mini-btn icon" title=move || t!("match-move-up-title") disabled=ci == 0 on:click=move |_| edit.run(EditFn::new(move |s, _| s.cases.swap(ci, ci - 1)))>"↑"</button>
            <button
                class="mini-btn icon"
                title=move || t!("match-move-down-title")
                // Expressions with `<` / `>` must be braced so `view!` doesn't parse them as tag endings
                prop:disabled={move || ci + 1 >= count()}
                on:click={move |_| edit.run(EditFn::new(move |s, _| if ci + 1 < s.cases.len() { s.cases.swap(ci, ci + 1) }))}
            >"↓"</button>
            <button
                class="mini-btn"
                title=move || t!("match-add-condition-title")
                on:click=move |_| edit.run(EditFn::new(move |s, env| {
                    let c = new_condition(s, env, Join::And);
                    s.cases[ci].conditions.push(c);
                }))
            >{move || t!("match-add-condition")}</button>
            <button
                class="mini-btn icon danger"
                title=move || t!("match-remove-case-title")
                on:click=move |_| edit.run(EditFn::new(move |s, _| { s.cases.remove(ci); }))
            >
                <Icon path=icons::DELETE />
            </button>
            {move || out.get().map(|p| view! { <Pin node=node pin=p dir=PinDirection::Output label=Signal::derive(move || t!("match-then", index = ci + 1)) /> })}
        </div>
        <For each=move || 0..rows() key=|ri| *ri let:ri>
            <ConditionRow case=case state=state types=types ci=ci ri=ri edit=edit />
        </For>
        {move || (rows() == 0).then(|| view! { <div class="case-row cond-row always">{move || t!("match-always")}</div> })}
    }
}

/// Right-hand side of a condition: none (unary operator), a constant value, or another input.
#[derive(Debug, Clone, PartialEq)]
enum Right {
    Value(DataType),
    Input(String),
}

#[component]
fn ConditionRow(case: Memo<Option<MatchCase>>, state: Memo<MatchState>, types: Memo<InputTypes>, ci: usize, ri: usize, edit: Callback<EditFn>) -> impl IntoView {
    let ed = use_editor();
    let cond = Memo::new(move |_| case.with(|c| c.as_ref().and_then(|c| c.conditions.get(ri).cloned())));
    let join = move || cond.with(|c| c.as_ref().map(|c| c.join));
    let left_type = Memo::new(move |_| {
        let left = cond.with(|c| c.as_ref().map(|c| c.left.clone()));
        left.map_or(DataType::Any, |l| types.with(|t| type_of(t, &l)))
    });
    let left_text = Memo::new(move |_| cond.with(|c| c.as_ref().map(|c| match &c.left {
        Operand::Input(n) => n.clone(),
        other => other.to_text(),
    }).unwrap_or_default()));
    let names = Memo::new(move |_| state.with(|s| s.inputs.iter().map(|i| i.name.clone()).collect::<Vec<_>>()));
    let op = Memo::new(move |_| cond.with(|c| c.as_ref().map(|c| c.op)));
    let right = Memo::new(move |_| {
        let (op, right) = cond.with(|c| c.as_ref().map(|c| (c.op, c.right.clone())))?;
        let rt = op.right_type(&left_type.get())?;
        Some(match right {
            Some(Operand::Input(n)) => Right::Input(n),
            _ => Right::Value(rt),
        })
    });
    let right_value = Signal::derive(move || cond.with(|c| match c.as_ref().and_then(|c| c.right.as_ref()) {
        Some(Operand::Value(v)) => Some(v.clone()),
        _ => None,
    }));

    let set_left = move |name: String| edit.run(EditFn::new(move |s, env| {
        let c = &mut s.cases[ci].conditions[ri];
        c.left = Operand::Input(name);
        let t = type_of(&env.types, &c.left);
        fit_condition(&env.reg, c, &t);
    }));
    let set_op = move |op: CompareOp| edit.run(EditFn::new(move |s, env| {
        let c = &mut s.cases[ci].conditions[ri];
        c.op = op;
        let t = type_of(&env.types, &c.left);
        fit_condition(&env.reg, c, &t);
    }));
    let set_right = move |o: Option<Operand>| edit.run(EditFn::new(move |s, env| {
        let c = &mut s.cases[ci].conditions[ri];
        c.right = o;
        let t = type_of(&env.types, &c.left);
        fit_condition(&env.reg, c, &t);
    }));

    view! {
        <div class="case-row cond-row">
            {if ri == 0 {
                view! { <span class="join-label">{move || t!("match-if")}</span> }.into_any()
            } else {
                view! {
                    <button
                        class=move || format!("join-btn{}", if join() == Some(Join::Or) { " or" } else { "" })
                        title=move || t!("match-join-title")
                        on:click=move |_| {
                            let next = if join() == Some(Join::Or) { Join::And } else { Join::Or };
                            edit.run(EditFn::new(move |s, _| s.cases[ci].conditions[ri].join = next));
                        }
                    >
                        {move || join().map(i18n::join_symbol).unwrap_or_default()}
                    </button>
                }
                .into_any()
            }}
            <select
                class="cond-left"
                title=move || t!("match-left-input-title", type = ed.type_title(&left_type.get()))
                style=move || format!("--pin-color:{}", ed.types.with(|r| type_color(r, &left_type.get())))
                on:change=move |ev| {
                    let name = event_target_value(&ev);
                    if is_input_name(&name) {
                        set_left(name);
                    }
                }
            >
                {move || {
                    let current = left_text.get();
                    let mut list = names.get();
                    // Legacy conditions may have a constant on the left or reference a non-existent input
                    if !list.contains(&current) {
                        list.insert(0, current.clone());
                    }
                    list.into_iter().map(|n| {
                        let selected = n == current;
                        view! { <option value=n.clone() selected=selected>{n.clone()}</option> }
                    }).collect_view()
                }}
            </select>
            <select
                class="op-select cond-op"
                title=move || t!("match-op-title")
                on:change=move |ev| {
                    if let Some(op) = CompareOp::parse(&event_target_value(&ev)) {
                        set_op(op);
                    }
                }
            >
                {move || {
                    let t = left_type.get();
                    let current = op.get();
                    let mut ops = ed.types.with(|r| ops_in(r, &t)).to_vec();
                    if let Some(c) = current.filter(|c| !ops.contains(c)) {
                        ops.insert(0, c);
                    }
                    ops.into_iter().map(|o| view! {
                        <option value=o.text() selected=Some(o) == current>{i18n::op_label_for(o, &t)}</option>
                    }).collect_view()
                }}
            </select>
            {move || match right.get() {
                None => view! { <span class="cond-fill"></span> }.into_any(),
                Some(r) => {
                    let is_value = matches!(r, Right::Value(_));
                    let source = view! {
                        <select
                            class=if is_value { "cond-src value" } else { "cond-src" }
                            title=move || t!("match-right-source-title")
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                // An empty right operand will be replaced with a default value by fit_condition
                                set_right(if v.is_empty() { None } else { Some(Operand::Input(v)) });
                            }
                        >
                            <option value="" selected=is_value>"✎"</option>
                            {names.get().into_iter().map(|n| {
                                let selected = matches!(&r, Right::Input(x) if *x == n);
                                view! { <option value=n.clone() selected=selected>{n.clone()}</option> }
                            }).collect_view()}
                        </select>
                    };
                    let value = match &r {
                        Right::Value(rt) => {
                            let vt = if ed.types.with(|r| text::is_text_type_in(r, rt)) { rt.clone() } else { DataType::Any };
                            Some(view! {
                                <InlineValue
                                    value=right_value
                                    value_type=vt
                                    title=Signal::derive(move || t!("match-right-value-title"))
                                    unit_literal=true
                                    on_commit=Callback::new(move |v: Option<DataValue>| {
                                        if let Some(v) = v {
                                            set_right(Some(Operand::Value(v)));
                                        }
                                    })
                                />
                            })
                        }
                        Right::Input(_) => None,
                    };
                    view! { {source} {value} }.into_any()
                }
            }}
            <button
                class="mini-btn icon"
                title=move || t!("match-remove-condition-title")
                on:click=move |_| edit.run(EditFn::new(move |s, _| { s.cases[ci].conditions.remove(ri); }))
            >
                <Icon path=icons::CLOSE />
            </button>
        </div>
    }
}
