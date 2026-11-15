use crate::{Node, NodeContext};
use ticknode_shared::*;

/// RS trigger (flip-flop) active node.
///
/// Pin IDs:
/// - Inputs:
///   - 0: `Set` (Exec) — pulse turns the state on
///   - 1: `Reset` (Exec) — pulse turns the state off
///   - 2: `Toggle` (Exec) — pulse inverts the state
///   - 3: `S` (Data, Bool, optional) — level input for set
///   - 4: `R` (Data, Bool, optional) — level input for reset
///   - 5: `Evaluate` (Exec) — re-evaluates S and R level inputs
/// - Outputs:
///   - 0: `State` (Data, Bool) — current boolean state
///   - 1: `Turned on` (Exec) — fired on transition from false to true
///   - 2: `Turned off` (Exec) — fired on transition from true to false
///   - 3: `Out` (Exec) — fired on every evaluation
pub struct RsTriggerNode {
    pub id: NodeId,
    priority_set: bool,
    initial_on: bool,
    state: bool,
    last_tick: Option<u64>,
    exec_set_this_tick: bool,
    exec_reset_this_tick: bool,
}

impl RsTriggerNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            priority_set: false,
            initial_on: false,
            state: false,
            last_tick: None,
            exec_set_this_tick: false,
            exec_reset_this_tick: false,
        }
    }

    fn update_tick_pulses(&mut self, current_tick: u64, trigger_pin: Option<PinId>) {
        if self.last_tick != Some(current_tick) {
            self.last_tick = Some(current_tick);
            self.exec_set_this_tick = false;
            self.exec_reset_this_tick = false;
        }
        if trigger_pin == Some(PinId(0)) {
            self.exec_set_this_tick = true;
        } else if trigger_pin == Some(PinId(1)) {
            self.exec_reset_this_tick = true;
        }
    }
}

fn next_state(
    current: bool,
    is_set: bool,
    is_reset: bool,
    priority_set: bool,
    trigger_pin: Option<PinId>,
) -> bool {
    if is_set && is_reset {
        priority_set
    } else if is_set {
        true
    } else if is_reset {
        false
    } else if trigger_pin == Some(PinId(2)) {
        !current
    } else {
        current
    }
}

impl Node for RsTriggerNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn definition(&self) -> NodeDefinition {
        let exec = |id, name: &str, direction| PinDefinition {
            id: PinId(id),
            name: name.into(),
            pin_type: PinType::Exec,
            direction,
            required: false,
        };
        NodeDefinition {
            type_name: "logic/rs_trigger".into(),
            title: "RS Trigger".into(),
            category: "Logic".into(),
            description: "Bistable flip-flop with pulse and level inputs".into(),
            help: "Bistable trigger (flip-flop) that holds a boolean state.\n\nExec inputs provide pulse-driven control: Set turns the state on, Reset turns it off, and Toggle inverts the current state.\n\nData inputs S (Set) and R (Reset) provide level-driven control: they are evaluated whenever the node runs on any exec input. A pulse at Evaluate re-evaluates S and R without an accompanying Set, Reset, or Toggle pulse. While S is true and R is false, the trigger turns on. While R is true and S is false, it turns off. When neither S nor R is true, the trigger latches and keeps its previous state.\n\nWhen both Set and Reset are active at the same time — whether both level inputs S and R are true, or Set and Reset exec pulses arrive in the same tick — the Priority property determines the winning state (Reset by default, or Set).\n\nThe Initial state property chooses whether the trigger starts in the off (default) or on state after creation or server restart. State lives in memory and is not persisted across restarts.\n\nEvery evaluation emits State first, then Turned on or Turned off only if the state changed, then Out.".into(),
            adaptive: false,
            cost: 1,
            properties: vec![
                PropertyDefinition::select("priority", "Priority", &[
                    ("reset", "Reset"),
                    ("set", "Set"),
                ])
                .described("Which state wins when both set and reset are active at the same time"),
                PropertyDefinition::select("initial", "Initial state", &[
                    ("off", "Off"),
                    ("on", "On"),
                ])
                .described("State of the trigger after creation or restart"),
            ],
            pins: vec![
                exec(0, "Set", PinDirection::Input),
                exec(1, "Reset", PinDirection::Input),
                exec(2, "Toggle", PinDirection::Input),
                PinDefinition {
                    id: PinId(3),
                    name: "S".into(),
                    pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(4),
                    name: "R".into(),
                    pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Input,
                    required: false,
                },
                exec(5, "Evaluate", PinDirection::Input),
                PinDefinition {
                    id: PinId(0),
                    name: "State".into(),
                    pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Output,
                    required: false,
                },
                exec(1, "Turned on", PinDirection::Output),
                exec(2, "Turned off", PinDirection::Output),
                exec(3, "Out", PinDirection::Output),
            ],
        }
    }

    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        self.update_tick_pulses(ctx.host_state.current_tick, trigger_pin);

        let s_level = ctx.get_bool(PinId(3), false);
        let r_level = ctx.get_bool(PinId(4), false);
        let is_set = self.exec_set_this_tick || s_level;
        let is_reset = self.exec_reset_this_tick || r_level;

        let previous = self.state;
        self.state = next_state(previous, is_set, is_reset, self.priority_set, trigger_pin);

        let mut outputs = vec![NodeOutput::Data(PinId(0), DataValue::Bool(self.state))];
        if self.state != previous {
            outputs.push(NodeOutput::Execution(if self.state {
                PinId(1)
            } else {
                PinId(2)
            }));
        }
        outputs.push(NodeOutput::Execution(PinId(3)));
        outputs
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            (
                "priority".into(),
                if self.priority_set { "set" } else { "reset" }.into(),
            ),
            (
                "initial".into(),
                if self.initial_on { "on" } else { "off" }.into(),
            ),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let DataValue::String(value) = value else {
            return;
        };
        match name {
            "priority" if value == "reset" || value == "set" => {
                self.priority_set = value == "set";
            }
            "initial" if value == "off" || value == "on" => {
                let on = value == "on";
                self.initial_on = on;
                self.state = on;
            }
            _ => {}
        }
    }
}
