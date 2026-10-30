//! Нода Match прямо на ноді — як вікно комбінатора-вирішувача Factorio. Згори вниз:
//! вхід In; рядки входів (сокет, ім'я, тип, значення); «+ вхід»; далі випадки — заголовок зі своїм
//! exec-виходом і рядки умов «вхід  оператор  операнд», де оператори залежать від типу входу;
//! наприкінці «+ випадок» і вихід Else. Кожна зміна одразу надсилається на сервер (властивості
//! `inputs` і `cases`); стан — лише з сервера. Висоти рядків — `geometry::NodeLayout`.

use leptos::prelude::*;
use ticknode_shared::condition::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;

use crate::canvas::Pin;
use crate::geometry::{match_cases, match_inputs_of, type_color};
use crate::i18n::{self, t};
use crate::inline::{InlineValue, PinValue};
use crate::state::{ClientNode, Editor, LogKind, use_editor};
use crate::ui::{ControlState, Icon, flash, icons};

/// Входи й випадки разом: перейменування чи видалення входу змінює й умови.
#[derive(Debug, Clone, PartialEq)]
struct MatchState {
    inputs: Vec<MatchInput>,
    cases: Vec<MatchCase>,
}

/// Ефективні типи входів: для «auto» — тип під'єднаного джерела чи значення на ноді.
/// Пін Else ноди Match (як `MatchNode::ELSE_PIN` у ядрі).
const ELSE_PIN: PinId = PinId(0);

type InputTypes = Vec<(String, DataType)>;

fn type_of(types: &[(String, DataType)], operand: &Operand) -> DataType {
    match operand {
        Operand::Input(name) => types.iter().find(|(n, _)| n == name).map_or(DataType::Any, |(_, t)| t.clone()),
        Operand::Value(v) => v.get_type(),
    }
}

fn effective_type(ed: Editor, id: NodeId, node: ClientNode, input: &MatchInput) -> DataType {
    if input.value_type != DataType::Any {
        return input.value_type.clone();
    }
    let pin = PinId(input.pin);
    if ed.is_input_connected(id, pin) {
        return ed.source_type(id, pin).unwrap_or(DataType::Any);
    }
    let prop = input_value_property(pin);
    node.instance
        .with(|i| i.model.properties.iter().find(|(n, _)| *n == prop).map(|(_, v)| v.get_type()))
        .unwrap_or(DataType::Any)
}

/// Підганяє умову під тип лівого входу: оператор — з доречних, правий операнд — потрібного типу
/// (значення по змозі перетворюється, інакше — типове).
fn fit_condition(reg: &TypeRegistry, c: &mut Condition, left: &DataType) {
    let ops = ops_in(reg, left);
    if !ops.contains(&c.op) {
        c.op = ops[0];
    }
    let Some(rt) = c.op.right_type(left) else {
        c.right = None;
        return;
    };
    let fits = match &c.right {
        Some(Operand::Input(_)) => true,
        Some(Operand::Value(v)) => rt == DataType::Any || input_value_fits(reg, &rt, v) || reg.compatible(&v.get_type(), &rt),
        None => false,
    };
    if !fits {
        let converted = match &c.right {
            Some(Operand::Value(v)) if text::is_text_type_in(reg, &rt) => text::parse_value_in(reg, &text::format_value(v), &rt).ok(),
            _ => None,
        };
        let default = || match reg.default_enum(&rt) {
            Some(first) => DataValue::String(first),
            None => text::default_of(&rt),
        };
        c.right = Some(Operand::Value(converted.unwrap_or_else(default)));
    }
}

/// Перше вільне ім'я: a, b, c, … потім in1, in2, …
fn free_name(inputs: &[MatchInput]) -> String {
    let taken = |n: &str| inputs.iter().any(|i| i.name == n);
    ('a'..='z').map(String::from)
        .chain((1..).map(|i| format!("in{i}")))
        .find(|n| !taken(n))
        .expect("нескінченний ряд імен")
}

fn add_input(s: &mut MatchState) -> String {
    let pin = s.inputs.iter().map(|i| i.pin).max().unwrap_or(0) + 1;
    let name = free_name(&s.inputs);
    s.inputs.push(MatchInput { pin, name: name.clone(), value_type: DataType::Any });
    name
}

/// Нова умова — на перший вхід (без входів — спершу додається вхід).
fn new_condition(s: &mut MatchState, env: &Env, join: Join) -> Condition {
    let types = &env.types;
    let name = match s.inputs.first() {
        Some(i) => i.name.clone(),
        None => add_input(s),
    };
    let left = Operand::Input(name);
    let t = type_of(types, &left);
    let mut c = Condition { join, left, op: ops_in(&env.reg, &t)[0], right: None };
    fit_condition(&env.reg, &mut c, &t);
    c
}

/// Що знають правки: ефективні типи входів і реєстр власних типів.
pub struct Env {
    types: InputTypes,
    reg: TypeRegistry,
}

type MatchEdit = Box<dyn FnOnce(&mut MatchState, &Env)>;

/// Зміна стану ноди (обгортка, щоб передавати через `Callback`). Другий аргумент — ефективні типи.
pub struct EditFn(MatchEdit);

impl EditFn {
    fn new(f: impl FnOnce(&mut MatchState, &Env) + 'static) -> Self {
        Self(Box::new(f))
    }
}

/// Натискання на поле чи кнопку не починає перетягування ноди; на пін чи фон — як завжди.
pub fn stop_on_controls(ev: web_sys::PointerEvent) {
    let on_control = ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .and_then(|el| el.closest("input, select, button").ok().flatten())
        .is_some();
    if on_control {
        ev.stop_propagation();
    }
}

fn pin_def(id: NodeId, pin: PinId, dir: PinDirection) -> Option<PinDefinition> {
    let ed = use_editor();
    ed.node_tracked(id)?.instance.with(|i| i.definition.pins.iter().find(|p| p.id == pin && p.direction == dir).cloned())
}

fn string_list(items: impl Iterator<Item = String>) -> DataValue {
    DataValue::List { element_type: DataType::String, elements: items.map(DataValue::String).collect() }
}

#[component]
pub fn MatchBody(id: NodeId, #[prop(into)] top: Signal<f64>) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else { return ().into_any() };
    let state = Memo::new(move |_| node.instance.with(|i| MatchState {
        inputs: match_inputs_of(&i.model.properties),
        cases: match_cases(&i.model.properties),
    }));
    let types = Memo::new(move |_| {
        let inputs = state.with(|s| s.inputs.clone());
        inputs.iter().map(|i| (i.name.clone(), effective_type(ed, id, node, i))).collect::<InputTypes>()
    });
    // Поле може втратити фокус (і застосувати зміну) уже після того, як його прибрали,
    // тож стан читаємо через try_
    let edit = Callback::new(move |f: EditFn| {
        let (Some(before), Some(types)) = (state.try_get_untracked(), types.try_get_untracked()) else { return };
        let mut after = before.clone();
        f.0(&mut after, &Env { types, reg: ed.types.get_untracked() });
        // Входи — першими: умови можуть посилатися на щойно доданий вхід
        if after.inputs != before.inputs {
            ed.send(ClientMessage::SetProperty { id, name: "inputs".into(), value: string_list(after.inputs.iter().map(MatchInput::to_text)) });
        }
        if after.cases != before.cases {
            ed.send(ClientMessage::SetProperty { id, name: "cases".into(), value: string_list(after.cases.iter().map(MatchCase::to_text)) });
        }
    });
    let in_pin = Memo::new(move |_| pin_def(id, PinId(0), PinDirection::Input));
    let else_pin = Memo::new(move |_| pin_def(id, ELSE_PIN, PinDirection::Output));

    view! {
        <div class="match-body" style=move || format!("top:{}px", top.get()) on:pointerdown=stop_on_controls>
            <div class="case-row match-in">
                {move || in_pin.get().map(|p| view! { <Pin node=id pin=p dir=PinDirection::Input /> })}
                <span class="match-section">{move || t!("match-section-inputs")}</span>
            </div>
            <For each=move || state.with(|s| s.inputs.iter().enumerate().map(|(i, inp)| (i, inp.pin)).collect::<Vec<_>>()) key=|k| *k let:key>
                <InputRow node=id state=state types=types ii=key.0 pin=key.1 edit=edit />
            </For>
            <div class="case-row input-foot">
                <button
                    class="mini-btn"
                    title=move || t!("match-add-input-title")
                    on:click=move |_| edit.run(EditFn::new(|s, _| { add_input(s); }))
                >
                    {move || t!("match-add-input")}
                </button>
                <span class="match-section">{move || t!("match-section-cases")}</span>
            </div>
            // Рядки живуть, поки не зміниться структура (ключі — номер і пін), тож фокус не губиться
            <For each=move || state.with(|s| s.cases.iter().enumerate().map(|(i, c)| (i, c.pin)).collect::<Vec<_>>()) key=|k| *k let:key>
                <CaseBlock node=id state=state types=types ci=key.0 pin=key.1 edit=edit />
            </For>
            <div class="case-row case-foot">
                <button
                    class="mini-btn"
                    title=move || t!("match-add-case-title")
                    on:click=move |_| edit.run(EditFn::new(|s, env| {
                        // Номер піна — новий, щоб з'єднання інших випадків не переїхали
                        let pin = s.cases.iter().map(|c| c.pin).max().unwrap_or(0) + 1;
                        let c = new_condition(s, env, Join::And);
                        s.cases.push(MatchCase { pin, conditions: vec![c] });
                    }))
                >
                    {move || t!("match-add-case")}
                </button>
                {move || else_pin.get().map(|p| view! { <Pin node=id pin=p dir=PinDirection::Output label=Signal::derive(move || t!("match-else")) /> })}
            </div>
        </div>
    }
    .into_any()
}

#[component]
fn InputRow(node: NodeId, state: Memo<MatchState>, types: Memo<InputTypes>, ii: usize, pin: u32, edit: Callback<EditFn>) -> impl IntoView {
    let ed = use_editor();
    let pin_id = PinId(pin);
    let input = Memo::new(move |_| state.with(|s| s.inputs.get(ii).cloned()));
    let name = Signal::derive(move || input.with(|i| i.as_ref().map(|i| i.name.clone()).unwrap_or_default()));
    let declared = Memo::new(move |_| input.with(|i| i.as_ref().map_or(DataType::Any, |i| i.value_type.clone())));
    let effective = Memo::new(move |_| types.with(|t| t.get(ii).map_or(DataType::Any, |(_, t)| t.clone())));
    let connected = Memo::new(move |_| ed.is_input_connected(node, pin_id));

    let rename = Callback::new(move |new: String| -> Result<(), String> {
        let new = new.trim().to_string();
        if !is_input_name(&new) {
            return Err(t!("match-error-bad-input-name", name = new));
        }
        if state.with_untracked(|s| s.inputs.iter().enumerate().any(|(j, i)| j != ii && i.name == new)) {
            return Err(t!("match-error-duplicate-input", name = new));
        }
        edit.run(EditFn::new(move |s, _| {
            let old = std::mem::replace(&mut s.inputs[ii].name, new.clone());
            for c in &mut s.cases {
                c.rename_input(&old, &new);
            }
        }));
        Ok(())
    });
    let set_type = move |t: DataType| edit.run(EditFn::new(move |s, env| {
        let types = &env.types;
        let name = s.inputs[ii].name.clone();
        s.inputs[ii].value_type = t.clone();
        // Для «auto» підходять оператори фактичного типу
        let left = if t == DataType::Any { type_of(types, &Operand::Input(name.clone())) } else { t };
        for c in s.cases.iter_mut().flat_map(|c| c.conditions.iter_mut()) {
            if c.left == Operand::Input(name.clone()) {
                fit_condition(&env.reg, c, &left);
            }
        }
    }));
    let remove = move |_| edit.run(EditFn::new(move |s, env| {
        let types = &env.types;
        let name = s.inputs.remove(ii).name;
        let gone = Operand::Input(name);
        for case in &mut s.cases {
            case.conditions.retain(|c| c.left != gone);
            for c in &mut case.conditions {
                if c.right.as_ref() == Some(&gone) {
                    c.right = None;
                    let t = type_of(types, &c.left);
                    fit_condition(&env.reg, c, &t);
                }
            }
        }
    }));
    let pin_title = move || format!("{}: {}", name.get(), ed.type_title(&effective.get()));
    view! {
        <div class="case-row input-row">
            <div class="pin in match-socket" title=pin_title data-pin=pin.to_string() data-node=node.0.to_string() data-dir="in" data-kind="data">
                <span class="socket" class:connected=move || connected.get() style=move || format!("--pin-color:{}", ed.types.with(|r| type_color(r, &effective.get())))></span>
            </div>
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
                    .chain(ed.types.with(|r| r.definitions()).into_iter().map(|d| (d.name.clone(), d.title.clone())))
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
    }
}

#[component]
fn CaseBlock(node: NodeId, state: Memo<MatchState>, types: Memo<InputTypes>, ci: usize, pin: u32, edit: Callback<EditFn>) -> impl IntoView {
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
                // Вирази з `<` / `>` — у дужках, інакше view! сприйме їх як кінець тегу
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

/// Що праворуч в умові: нічого (унарний оператор), стала чи інший вхід.
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
                    // Старі умови можуть мати сталу ліворуч або посилання на неіснуючий вхід
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
                                // Порожній правий операнд fit_condition замінить типовим значенням
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

/// Текстове поле, що застосовується по Enter чи втраті фокусу (Esc — скасувати). Відмова
/// `on_commit` повертає попередній текст і показує помилку в консолі.
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
                    // Застосовується при втраті фокусу
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(text: &str) -> Condition {
        MatchCase::parse(&format!("1: {text}")).unwrap().conditions.remove(0)
    }

    #[test]
    fn conditions_follow_input_type() {
        // Число → рядок: «>» не годиться, 5 стає "5"
        let mut c = cond("a > 5");
        fit_condition(&TypeRegistry::new(), &mut c, &DataType::String);
        assert_eq!(c.op, CompareOp::Eq);
        assert_eq!(c.right, Some(Operand::Value(DataValue::String("5".into()))));
        // «=» годиться й для логічного — лишається, значення стає true/false
        fit_condition(&TypeRegistry::new(), &mut c, &DataType::Bool);
        assert_eq!((c.op, c.right.clone()), (CompareOp::Eq, Some(Operand::Value(DataValue::Bool(true)))));
        // «>» для логічного не годиться — перший доречний, унарний «= true»
        let mut c = cond("a > 5");
        fit_condition(&TypeRegistry::new(), &mut c, &DataType::Bool);
        assert_eq!((c.op, c.right.clone()), (CompareOp::IsTrue, None));
        // Час: значення з'являється потрібного типу
        let mut c = cond("a > b");
        fit_condition(&TypeRegistry::new(), &mut c, &DataType::Time);
        assert_eq!(c.right, Some(Operand::Input("b".into())));
        let mut c = cond("a = \"07:30\"");
        fit_condition(&TypeRegistry::new(), &mut c, &DataType::Time);
        assert!(matches!(c.right, Some(Operand::Value(DataValue::Time(_)))));
    }

    #[test]
    fn free_names() {
        let i = |n: &str| MatchInput { pin: 1, name: n.into(), value_type: DataType::Any };
        assert_eq!(free_name(&[i("a"), i("c")]), "b");
    }
}
