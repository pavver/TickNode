//! In-node Match editor — similar to Factorio's decider combinator window. Top to bottom:
//! In pin; input rows (socket, name, type, value); "+ input"; then cases — a header with its own
//! exec output and condition rows "input  operator  operand", where operators depend on the input type;
//! finally "+ case" and the Else output. Every change is immediately sent to the server (`inputs` and
//! `cases` properties); state is server-driven only. Row heights match `geometry::NodeLayout`.

use leptos::prelude::*;
use ticknode_shared::condition::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;

use crate::canvas::Pin;
use crate::geometry::{match_cases, match_inputs_of};
use crate::i18n::{self, t};
use crate::state::{ClientNode, Editor, use_editor};
use crate::ui::{Icon, icons};

mod cases;
mod inputs;

use cases::CaseBlock;
use inputs::InputRow;

/// Inputs and cases together: renaming or removing an input also updates conditions.
#[derive(Debug, Clone, PartialEq)]
struct MatchState {
    inputs: Vec<MatchInput>,
    cases: Vec<MatchCase>,
}

/// Effective input types: for "auto", the type of the connected source or the on-node value.
/// Else pin of the Match node (matches `MatchNode::ELSE_PIN` in the core).
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

/// Adapts a condition to the left input's type: picks a valid operator and ensures the right operand
/// has the expected type (converting the value when possible, or falling back to the default).
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

/// First unused name: a, b, c, … then in1, in2, …
fn free_name(inputs: &[MatchInput]) -> String {
    let taken = |n: &str| inputs.iter().any(|i| i.name == n);
    ('a'..='z').map(String::from)
        .chain((1..).map(|i| format!("in{i}")))
        .find(|n| !taken(n))
        .expect("an endless sequence of names")
}

fn add_input(s: &mut MatchState) -> String {
    let pin = s.inputs.iter().map(|i| i.pin).max().unwrap_or(0) + 1;
    let name = free_name(&s.inputs);
    s.inputs.push(MatchInput { pin, name: name.clone(), value_type: DataType::Any });
    name
}

/// New condition targeting the first input (if there are no inputs, adds one first).
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

/// Context passed to edits: effective input types and the custom type registry.
pub struct Env {
    types: InputTypes,
    reg: TypeRegistry,
}

type MatchEdit = Box<dyn FnOnce(&mut MatchState, &Env)>;

/// Node state mutation (wrapper for passing via `Callback`). The second argument holds effective types.
pub struct EditFn(MatchEdit);

impl EditFn {
    fn new(f: impl FnOnce(&mut MatchState, &Env) + 'static) -> Self {
        Self(Box::new(f))
    }
}

/// Clicking a field or button does not start node dragging; clicking a pin or background behaves normally.
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
    // A field may lose focus (and commit its change) after already being removed,
    // so read state via try_
    let edit = Callback::new(move |f: EditFn| {
        let (Some(before), Some(types)) = (state.try_get_untracked(), types.try_get_untracked()) else { return };
        let mut after = before.clone();
        f.0(&mut after, &Env { types, reg: ed.types.get_untracked() });
        // Send inputs first: conditions may reference a newly added input
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
            // Rows stay alive until the structure changes (keyed by index and pin), preserving focus
            <For each=move || state.with(|s| s.cases.iter().enumerate().map(|(i, c)| (i, c.pin)).collect::<Vec<_>>()) key=|k| *k let:key>
                <CaseBlock node=id state=state types=types ci=key.0 pin=key.1 edit=edit />
            </For>
            <div class="case-row case-foot">
                <button
                    class="mini-btn"
                    title=move || t!("match-add-case-title")
                    on:click=move |_| edit.run(EditFn::new(|s, env| {
                        // Allocate a fresh pin ID so existing case connections don't shift
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

#[cfg(test)]
mod tests;
