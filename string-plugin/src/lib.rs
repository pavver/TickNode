//! String manipulation operations.

use regex::Regex;
use ticknode_plugin_sdk::*;

struct StringPlugin;

#[derive(Clone, Copy)]
enum Op { Concat, Length, Substring, Contains, Split, Regex, Aggregate }

fn string_list() -> DataType {
    DataType::List(Box::new(DataType::String))
}

impl Op {
    fn from_type(type_name: &str) -> Option<Self> {
        Some(match type_name {
            "string/concat" => Op::Concat,
            "string/length" => Op::Length,
            "string/substring" => Op::Substring,
            "string/contains" => Op::Contains,
            "string/split" => Op::Split,
            "string/regex" => Op::Regex,
            "string/aggregate" => Op::Aggregate,
            _ => return None,
        })
    }

    /// Base pin ID: +0 — In, +1 — Out, followed by arguments and outputs.
    fn base(self) -> u32 {
        self as u32 * 10
    }
}

struct StringNode(Op);

impl PluginNode for StringNode {
    fn definition(&self) -> NodeDefinition {
        let b = self.0.base();
        let (type_name, title, description, data) = match self.0 {
            Op::Concat => ("string/concat", "Concat Strings", "Joins two strings", vec![
                data_in(b + 2, "A", DataType::String),
                data_in(b + 3, "B", DataType::String),
                data_out(b + 4, "Res", DataType::String),
            ]),
            Op::Length => ("string/length", "String Length", "Returns the length of a string", vec![
                data_in(b + 2, "Val", DataType::String),
                data_out(b + 3, "Res", DataType::Number),
            ]),
            Op::Substring => ("string/substring", "Substring", "Extracts a substring from a string", vec![
                data_in(b + 2, "Val", DataType::String),
                data_in(b + 3, "Start", DataType::Number),
                data_in(b + 4, "Len", DataType::Number),
                data_out(b + 5, "Res", DataType::String),
            ]),
            Op::Contains => ("string/contains", "String Contains", "Checks whether a string contains another substring", vec![
                data_in(b + 2, "Val", DataType::String),
                data_in(b + 3, "Sub", DataType::String),
                data_out(b + 4, "Res", DataType::Bool),
            ]),
            Op::Split => ("string/split", "Split String", "Splits a string by a separator", vec![
                data_in(b + 2, "Val", DataType::String),
                data_in(b + 3, "Pat", DataType::String),
                data_out(b + 4, "Res", string_list()),
            ]),
            Op::Regex => ("string/regex", "Regex Match", "Matches a regular expression and returns the match", vec![
                data_in(b + 2, "Val", DataType::String),
                data_in(b + 3, "Pat", DataType::String),
                data_out(b + 4, "Matched", DataType::Bool),
                data_out(b + 5, "Capture", DataType::String),
            ]),
            Op::Aggregate => ("string/aggregate", "Aggregate Strings", "Joins a list of strings with a separator", vec![
                data_in(b + 2, "List", string_list()),
                data_in(b + 3, "Sep", DataType::String),
                data_out(b + 4, "Res", DataType::String),
            ]),
        };
        let mut pins = vec![exec_in(b, "In"), exec_out(b + 1, "Out")];
        pins.extend(data);
        NodeDefinition {
            type_name: type_name.into(),
            title: title.into(),
            category: "String".into(),
            description: description.into(),
            help: match self.0 {
                Op::Concat => "Res is string A immediately followed by B.",
                Op::Length => "Res is the number of characters (not bytes) in string Val.",
                Op::Substring => "Res is Len characters of string Val starting at position Start (counting from 0). Anything beyond the end of the string is discarded.",
                Op::Contains => "Res = true if Val contains the substring Sub. An empty Sub is contained in any string.",
                Op::Split => "Res is a list of the parts of string Val split by the separator Pat. An empty Pat splits the string into single characters.",
                Op::Regex => "Searches Val for the regular expression Pat (Rust regex syntax):\n• Matched — whether a match was found;\n• Capture — the first parenthesized group, or the whole match if there are no groups.\n\nAn invalid expression gives Matched = false.",
                Op::Aggregate => "Res is the elements of List joined into one string with Sep. Non-string elements are skipped.",
            }.to_string() + "\n\nAn unconnected string input is treated as an empty string.\n\nRuns on a signal at In, then passes execution to Out — the result is available to the following nodes.",
            pins,
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let b = self.0.base();
        if trigger != Trigger::Pin(PinId(b)) {
            return Vec::new();
        }
        let s = |offset: u32| inputs.string(PinId(b + offset));
        let out = |offset: u32, value: DataValue| Output::Data(PinId(b + offset), value);
        let mut outputs = match self.0 {
            Op::Concat => vec![out(4, DataValue::String(s(2) + &s(3)))],
            Op::Length => vec![out(3, DataValue::Number(NumberValue::Int(s(2).chars().count() as i64)))],
            Op::Substring => {
                let start = inputs.number(PinId(b + 3), 0.0).max(0.0) as usize;
                let len = inputs.number(PinId(b + 4), 0.0).max(0.0) as usize;
                vec![out(5, DataValue::String(s(2).chars().skip(start).take(len).collect()))]
            }
            Op::Contains => vec![out(4, DataValue::Bool(s(2).contains(&s(3))))],
            Op::Split => {
                let (val, pat) = (s(2), s(3));
                let elements = if pat.is_empty() {
                    val.chars().map(|c| DataValue::String(c.to_string())).collect()
                } else {
                    val.split(&pat).map(|p| DataValue::String(p.to_string())).collect()
                };
                vec![out(4, DataValue::List { element_type: DataType::String, elements })]
            }
            Op::Regex => {
                let val = s(2);
                let capture = Regex::new(&s(3)).ok().and_then(|re| {
                    let caps = re.captures(&val)?;
                    Some(caps.get(1).or_else(|| caps.get(0)).map(|m| m.as_str().to_string()).unwrap_or_default())
                });
                vec![
                    out(4, DataValue::Bool(capture.is_some())),
                    out(5, DataValue::String(capture.unwrap_or_default())),
                ]
            }
            Op::Aggregate => {
                let parts: Vec<String> = match inputs.get(PinId(b + 2)) {
                    Some(DataValue::List { elements, .. }) => elements.iter()
                        .filter_map(|e| match e {
                            DataValue::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                vec![out(4, DataValue::String(parts.join(&s(3))))]
            }
        };
        outputs.push(Output::Exec(PinId(b + 1)));
        outputs
    }
}

impl Plugin for StringPlugin {
    const NODE_TYPES: &'static [&'static str] = &[
        "string/concat", "string/length", "string/substring", "string/contains",
        "string/split", "string/regex", "string/aggregate",
    ];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[("uk", include_str!("../locales/uk.ftl"))];

    fn manifest() -> PluginManifest {
        PluginManifest {
            name: "string_plugin".into(),
            version: "0.2.0".into(),
            author: "TickNode".into(),
            requested_capabilities: vec![],
            settings: vec![],
        }
    }

    fn create(type_name: &str, _node: NodeId) -> Option<Box<dyn PluginNode>> {
        Op::from_type(type_name).map(|op| Box::new(StringNode(op)) as Box<dyn PluginNode>)
    }
}

export_plugin!(StringPlugin);
