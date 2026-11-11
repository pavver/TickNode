use ticknode_shared::condition::{MatchCase, MatchInput, match_inputs};
use ticknode_shared::*;
use crate::{Node, NodeContext};

/// How Match chooses which matching cases fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// The first matching case, or Else.
    #[default]
    First,
    /// Every matching case, or Else.
    All,
    /// The first matching case, but only when it differs from the case that fired last time.
    OnChange,
}

impl MatchMode {
    pub const OPTIONS: &[(&str, &str)] = &[
        ("first", "First match"),
        ("all", "All matches"),
        ("on_change", "Only on change"),
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::All => "all",
            Self::OnChange => "on_change",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "first" => Some(Self::First),
            "all" => Some(Self::All),
            "on_change" => Some(Self::OnChange),
            _ => None,
        }
    }
}

/// Decider combinator: on an In signal, evaluates cases in order and routes execution according to
/// [`MatchMode`], or to Else if none match.
/// Inputs are an explicit list (`inputs`: pin, name, type); in older graphs without it, names are inferred from conditions.
pub struct MatchNode {
    pub id: NodeId,
    pub cases: Vec<MatchCase>,
    pub mode: MatchMode,
    /// `OnChange` memory (runtime only): the case that fired last; kept while no case matches.
    last_case: Option<PinId>,
    /// `OnChange` memory (runtime only): Else already fired since the last matching case.
    in_else: bool,
    inputs: Vec<MatchInput>,
    /// Inputs are defined explicitly (otherwise inferred from conditions, as in legacy graphs).
    explicit_inputs: bool,
}

impl MatchNode {
    pub const ELSE_PIN: PinId = PinId(0);

    pub fn new(id: NodeId) -> Self {
        // Default input `a` is inferred from the condition. While the node is being restored, cases without `inputs`
        // (legacy format) infer inputs; after `restored`, inputs are explicit — both for new and loaded nodes.
        let mut node = Self { id, cases: Vec::new(), mode: MatchMode::First, last_case: None, in_else: false, inputs: Vec::new(), explicit_inputs: false };
        node.set_cases(vec![MatchCase::parse("1: a > 0").expect("the default case is valid")]);
        node
    }

    /// Case pin numbers are unique and do not collide with Else.
    fn set_cases(&mut self, mut cases: Vec<MatchCase>) {
        self.forget();
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
        self.forget();
        self.inputs = inputs;
        self.explicit_inputs = true;
    }

    fn set_mode(&mut self, mode: MatchMode) {
        self.mode = mode;
        self.forget();
    }

    /// Clears the `OnChange` memory, so the next matching case fires again.
    fn forget(&mut self) {
        self.last_case = None;
        self.in_else = false;
    }

    /// Exec outputs for the matching case pins (in case order) under the current mode.
    fn route(&mut self, matched: Vec<PinId>) -> Vec<NodeOutput> {
        let fired: Vec<PinId> = match (self.mode, matched.first().copied()) {
            (MatchMode::All, Some(_)) => matched,
            (MatchMode::First, Some(pin)) => vec![pin],
            (MatchMode::OnChange, Some(pin)) => {
                self.in_else = false;
                if self.last_case == Some(pin) {
                    return Vec::new();
                }
                self.last_case = Some(pin);
                vec![pin]
            }
            (MatchMode::OnChange, None) => {
                // The dead band between thresholds keeps the last case; Else fires once on entering it.
                if std::mem::replace(&mut self.in_else, true) {
                    return Vec::new();
                }
                vec![Self::ELSE_PIN]
            }
            (_, None) => vec![Self::ELSE_PIN],
        };
        fired.into_iter().map(NodeOutput::Execution).collect()
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
            adaptive: false,
            cost: 1,
            properties: vec![
                PropertyDefinition::select("mode", "Mode", MatchMode::OPTIONS)
                    .described("Which matching cases fire: the first one, all of them, or the first one only when it changes"),
            ],
            type_name: "match".to_string(),
            title: "Match".to_string(),
            category: "Control Flow".to_string(),
            description: "Selects an output by conditions, like a decider combinator".to_string(),
            help: "On a signal at In, checks cases from top to bottom and passes execution to the output of the first matching case. If none match — to Else.\n\nMode: \"First match\" fires only the first matching case; \"All matches\" fires every matching case; \"Only on change\" fires the first matching case only when it differs from the case that fired last time. When no case matches in that mode, Else fires once and the last case is remembered, so a gap between thresholds works as hysteresis: with cases \"v > 230\" and \"v < 200\", values between 200 and 230 do not let \"v > 230\" fire again until \"v < 200\" has fired. The memory resets when cases, inputs or the mode change and after a restart.\n\nInputs: \"+ input\" adds an input pin. The input name can be renamed (conditions update automatically) and its type selected; \"auto\" accepts anything, picking operators based on the connected source type. An unconnected input can be given a value directly on the node.\n\nA condition is \"input  operator  operand\". Operators depend on the input type:\n• numbers, time, duration — =, ≠, <, ≤, >, ≥ (for time — earlier / later);\n• strings — equals, contains, starts with, ends with, is empty, length;\n• booleans — = true / = false;\n• lists — contains, is empty, length;\n• objects — has field;\n• any — has value / no value.\nThe right side is a value or another input (comparing two variables). Values are coerced to the input type: \"07:30\" — time, \"1h 30m\" or 90 — duration, \"#ff0000\" — color.\n\nConditions are joined with \"and\" / \"or\"; \"and\" binds tighter: \"a > 1 and b > 1 or c\" = \"(a > 1 and b > 1) or c\". A case with no conditions always matches. An input without a value does not satisfy a condition (except \"no value\").".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value_of = |name: &str| {
            self.inputs.iter().find(|i| i.name == name).and_then(|i| ctx.get_value(PinId(i.pin)))
        };
        let all = self.mode == MatchMode::All;
        let mut matched = Vec::new();
        for case in &self.cases {
            if case.matches_in(ctx.types, |name| self.inputs.iter().find(|i| i.name == name).map_or(DataType::Any, |i| if i.value_type == DataType::Any { ctx.input_types.get(&PinId(i.pin)).cloned().unwrap_or(DataType::Any) } else { i.value_type.clone() }), value_of) {
                matched.push(PinId(case.pin));
                if !all {
                    break;
                }
            }
        }
        self.route(matched)
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
            ("mode".to_string(), DataValue::String(self.mode.as_str().to_string())),
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
            ("mode", DataValue::String(text)) => match MatchMode::parse(&text) {
                Some(mode) => self.set_mode(mode),
                None => tracing::warn!("Match: unknown mode \"{text}\", keeping \"{}\"", self.mode.as_str()),
            },
            // Graphs saved before `mode` existed
            ("all_matches", DataValue::Bool(all)) => self.set_mode(if all { MatchMode::All } else { MatchMode::First }),
            _ => {}
        }
    }
    fn restored(&mut self) {
        self.explicit_inputs = true;
    }
}
