//! Mathematical functions on numbers.

use ticknode_plugin_sdk::*;

struct MathPlugin;

#[derive(Clone, Copy)]
enum Op { Sin, Cos, Abs, Pow, Sqrt, Round, Min, Max }

impl Op {
    fn from_type(type_name: &str) -> Option<Self> {
        Some(match type_name {
            "math/sin" => Op::Sin,
            "math/cos" => Op::Cos,
            "math/abs" => Op::Abs,
            "math/pow" => Op::Pow,
            "math/sqrt" => Op::Sqrt,
            "math/round" => Op::Round,
            "math/min" => Op::Min,
            "math/max" => Op::Max,
            _ => return None,
        })
    }

    /// (type, title, description, argument names, base pin ID).
    /// Pin IDs: base+0 — In, +1 — Out, +2.. — arguments, followed by Res.
    fn info(self) -> (&'static str, &'static str, &'static str, &'static [&'static str], u32) {
        match self {
            Op::Sin => ("math/sin", "Sin", "Sine of an angle (in radians)", &["Val"], 0),
            Op::Cos => ("math/cos", "Cos", "Cosine of an angle (in radians)", &["Val"], 10),
            Op::Abs => ("math/abs", "Abs", "Absolute value of a number", &["Val"], 20),
            Op::Pow => ("math/pow", "Pow", "Raises a number to a power", &["Base", "Exp"], 30),
            Op::Sqrt => ("math/sqrt", "Sqrt", "Square root of a number", &["Val"], 40),
            Op::Round => ("math/round", "Round", "Rounds a number", &["Val"], 50),
            Op::Min => ("math/min", "Min", "Smaller of two numbers", &["A", "B"], 60),
            Op::Max => ("math/max", "Max", "Larger of two numbers", &["A", "B"], 70),
        }
    }

    fn apply(self, a: f64, b: f64) -> f64 {
        match self {
            Op::Sin => a.sin(),
            Op::Cos => a.cos(),
            Op::Abs => a.abs(),
            Op::Pow => a.powf(b),
            Op::Sqrt => a.sqrt(),
            Op::Round => a.round(),
            Op::Min => a.min(b),
            Op::Max => a.max(b),
        }
    }
}

struct MathNode(Op);

impl PluginNode for MathNode {
    fn definition(&self) -> NodeDefinition {
        let (type_name, title, description, args, base) = self.0.info();
        let mut pins = vec![exec_in(base, "In")];
        pins.extend(args.iter().enumerate().map(|(i, name)| data_in(base + 2 + i as u32, name, DataType::Number)));
        pins.push(exec_out(base + 1, "Out"));
        pins.push(data_out(base + 2 + args.len() as u32, "Res", DataType::Number));
        NodeDefinition {
            cost: 2,
            properties: Vec::new(),
            type_name: type_name.into(),
            title: title.into(),
            category: "Math".into(),
            description: description.into(),
            help: match self.0 {
                Op::Sin => "Res = sin(Val), the angle is in radians.",
                Op::Cos => "Res = cos(Val), the angle is in radians.",
                Op::Abs => "Res is the absolute value of Val (the number without its sign).",
                Op::Pow => "Res = Base raised to the power Exp.",
                Op::Sqrt => "Res is the square root of Val. For a negative number the result is NaN.",
                Op::Round => "Res is Val rounded to the nearest integer (.5 rounds away from zero).",
                Op::Min => "Res is the smaller of A and B.",
                Op::Max => "Res is the larger of A and B.",
            }.to_string() + "\n\nAn unconnected input is treated as zero.\n\nRuns on a signal at In, then passes execution to Out — the result is available to the following nodes.",
            pins,
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let (_, _, _, args, base) = self.0.info();
        if trigger != Trigger::Pin(PinId(base)) {
            return Vec::new();
        }
        let a = inputs.number(PinId(base + 2), 0.0);
        let b = inputs.number(PinId(base + 3), 0.0);
        let result = NumberValue::Float(self.0.apply(a, b)).normalize();
        vec![
            Output::Data(PinId(base + 2 + args.len() as u32), DataValue::Number(result)),
            Output::Exec(PinId(base + 1)),
        ]
    }
}

impl Plugin for MathPlugin {
    const NODE_TYPES: &'static [&'static str] =
        &["math/sin", "math/cos", "math/abs", "math/pow", "math/sqrt", "math/round", "math/min", "math/max"];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[("uk", include_str!("../locales/uk.ftl"))];

    fn manifest() -> PluginManifest {
        PluginManifest {
            name: "math_plugin".into(),
            version: "0.2.0".into(),
            author: "TickNode".into(),
            requested_capabilities: vec![],
            settings: vec![],
        }
    }

    fn create(type_name: &str, _node: NodeId) -> Option<Box<dyn PluginNode>> {
        Op::from_type(type_name).map(|op| Box::new(MathNode(op)) as Box<dyn PluginNode>)
    }
}

export_plugin!(MathPlugin);

#[cfg(test)]
mod tests;
