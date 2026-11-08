use ticknode_shared::condition::{MatchCase, MatchInput, match_inputs};
use ticknode_shared::*;
use crate::{Node, NodeContext};

/// Decider combinator: on an In signal, evaluates cases in order and routes execution to the output
/// of the first matching case (or all matching cases), or to Else if none match.
/// Inputs are an explicit list (`inputs`: pin, name, type); in older graphs without it, names are inferred from conditions.
pub struct MatchNode {
    pub id: NodeId,
    pub cases: Vec<MatchCase>,
    /// Fire all matching cases rather than only the first one.
    pub all_matches: bool,
    inputs: Vec<MatchInput>,
    /// Inputs are defined explicitly (otherwise inferred from conditions, as in legacy graphs).
    explicit_inputs: bool,
}

impl MatchNode {
    pub const ELSE_PIN: PinId = PinId(0);

    pub fn new(id: NodeId) -> Self {
        // Default input `a` is inferred from the condition. While the node is being restored, cases without `inputs`
        // (legacy format) infer inputs; after `restored`, inputs are explicit — both for new and loaded nodes.
        let mut node = Self { id, cases: Vec::new(), all_matches: false, inputs: Vec::new(), explicit_inputs: false };
        node.set_cases(vec![MatchCase::parse("1: a > 0").expect("the default case is valid")]);
        node
    }

    /// Case pin numbers are unique and do not collide with Else.
    fn set_cases(&mut self, mut cases: Vec<MatchCase>) {
        let mut seen = std::collections::HashSet::new();
        cases.retain(|c| c.pin != Self::ELSE_PIN.0 && seen.insert(c.pin));
        self.cases = cases;
        if !self.explicit_inputs {
            self.inputs = match_inputs(&self.cases).into_iter().enumerate()
                .map(|(i, name)| MatchInput { pin: i as u32 + 1, name, value_type: DataType::Any })
                .collect();
        }
    }

    /// Input pins and names are unique; pin 0 is reserved for the In exec input.
    fn set_inputs(&mut self, mut inputs: Vec<MatchInput>) {
        let mut pins = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        inputs.retain(|i| i.pin != 0 && pins.insert(i.pin) && names.insert(i.name.clone()));
        self.inputs = inputs;
        self.explicit_inputs = true;
    }
}

fn string_list(elements: &[DataValue]) -> Option<Vec<&str>> {
    elements.iter().map(|e| match e {
        DataValue::String(s) => Some(s.as_str()),
        _ => None,
    }).collect()
}

impl Node for MatchNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false }];
        pins.extend(self.inputs.iter().map(|i| PinDefinition {
            id: PinId(i.pin),
            name: i.name.clone(),
            pin_type: PinType::Data(i.value_type.clone()),
            direction: PinDirection::Input, required: false }));
        pins.extend(self.cases.iter().map(|c| PinDefinition {
            id: PinId(c.pin),
            name: c.display(),
            pin_type: PinType::Exec,
            direction: PinDirection::Output, required: false }));
        pins.push(PinDefinition { id: Self::ELSE_PIN, name: "Else".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false });
        NodeDefinition {
            properties: Vec::new(),
            type_name: "match".to_string(),
            title: "Match".to_string(),
            category: "Control Flow".to_string(),
            description: "Selects an output by conditions, like a decider combinator".to_string(),
            help: "On a signal at In, checks cases from top to bottom and passes execution to the output of the first matching case. If none match — to Else. With \"All matches\" enabled, all matching cases fire.\n\nInputs: \"+ input\" adds an input pin. The input name can be renamed (conditions update automatically) and its type selected; \"auto\" accepts anything, picking operators based on the connected source type. An unconnected input can be given a value directly on the node.\n\nA condition is \"input  operator  operand\". Operators depend on the input type:\n• numbers, time, duration — =, ≠, <, ≤, >, ≥ (for time — earlier / later);\n• strings — equals, contains, starts with, ends with, is empty, length;\n• booleans — = true / = false;\n• lists — contains, is empty, length;\n• objects — has field;\n• any — has value / no value.\nThe right side is a value or another input (comparing two variables). Values are coerced to the input type: \"07:30\" — time, \"1h 30m\" or 90 — duration, \"#ff0000\" — color.\n\nConditions are joined with \"and\" / \"or\"; \"and\" binds tighter: \"a > 1 and b > 1 or c\" = \"(a > 1 and b > 1) or c\". A case with no conditions always matches. An input without a value does not satisfy a condition (except \"no value\").".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value_of = |name: &str| {
            self.inputs.iter().find(|i| i.name == name).and_then(|i| ctx.get_value(PinId(i.pin)))
        };
        let mut outputs = Vec::new();
        for case in &self.cases {
            if case.matches(value_of) {
                outputs.push(NodeOutput::Execution(PinId(case.pin)));
                if !self.all_matches {
                    break;
                }
            }
        }
        if outputs.is_empty() {
            outputs.push(NodeOutput::Execution(Self::ELSE_PIN));
        }
        outputs
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        let list = |items: Vec<String>| DataValue::List {
            element_type: DataType::String,
            elements: items.into_iter().map(DataValue::String).collect(),
        };
        // Inputs come before cases: when reconstructing the node, cases need not infer inputs on their own
        vec![
            ("inputs".to_string(), list(self.inputs.iter().map(MatchInput::to_text).collect())),
            ("cases".to_string(), list(self.cases.iter().map(MatchCase::to_text).collect())),
            ("all_matches".to_string(), DataValue::Bool(self.all_matches)),
        ]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        // An invalid list is not applied at all (the editor validates it beforehand)
        match (name, value) {
            ("cases", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "a case must be a string".to_string())
                    .and_then(|l| l.into_iter().map(|t| MatchCase::parse(t).map_err(|e| e.to_string())).collect());
                match parsed {
                    Ok(cases) => self.set_cases(cases),
                    Err(e) => tracing::warn!("Match: invalid cases: {e}"),
                }
            }
            ("inputs", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "an input must be a string".to_string())
                    .and_then(|l| l.into_iter().map(|t| MatchInput::parse(t).map_err(|e| e.to_string())).collect());
                match parsed {
                    Ok(inputs) => self.set_inputs(inputs),
                    Err(e) => tracing::warn!("Match: invalid inputs: {e}"),
                }
            }
            ("all_matches", DataValue::Bool(b)) => self.all_matches = b,
            _ => {}
        }
    }
    fn restored(&mut self) {
        self.explicit_inputs = true;
    }
}
