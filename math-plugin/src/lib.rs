//! Математичні функції над числами.

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

    /// (тип, назва, опис, назви аргументів, базовий ID пінів).
    /// ID пінів: база+0 — In, +1 — Out, +2.. — аргументи, далі — Res.
    fn info(self) -> (&'static str, &'static str, &'static str, &'static [&'static str], u32) {
        match self {
            Op::Sin => ("math/sin", "Sin", "Синус кута (в радіанах)", &["Val"], 0),
            Op::Cos => ("math/cos", "Cos", "Косинус кута (в радіанах)", &["Val"], 10),
            Op::Abs => ("math/abs", "Abs", "Абсолютне значення числа", &["Val"], 20),
            Op::Pow => ("math/pow", "Pow", "Зводить число в ступінь", &["Base", "Exp"], 30),
            Op::Sqrt => ("math/sqrt", "Sqrt", "Квадратний корінь числа", &["Val"], 40),
            Op::Round => ("math/round", "Round", "Округлення числа", &["Val"], 50),
            Op::Min => ("math/min", "Min", "Мінімальне з двох чисел", &["A", "B"], 60),
            Op::Max => ("math/max", "Max", "Максимальне з двох чисел", &["A", "B"], 70),
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
            type_name: type_name.into(),
            title: title.into(),
            category: "Math".into(),
            description: description.into(),
            help: match self.0 {
                Op::Sin => "Res = sin(Val), кут у радіанах.",
                Op::Cos => "Res = cos(Val), кут у радіанах.",
                Op::Abs => "Res — модуль Val (число без знака).",
                Op::Pow => "Res = Base у степені Exp.",
                Op::Sqrt => "Res — квадратний корінь Val. Для від'ємного числа результат — NaN.",
                Op::Round => "Res — Val, округлене до найближчого цілого (.5 — від нуля).",
                Op::Min => "Res — менше з чисел A і B.",
                Op::Max => "Res — більше з чисел A і B.",
            }.to_string() + "\n\nНепід'єднаний вхід вважається нулем.\n\nВиконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.",
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

    fn manifest() -> PluginManifest {
        PluginManifest {
            name: "math_plugin".into(),
            version: "0.2.0".into(),
            author: "TickNode".into(),
            requested_capabilities: vec![],
            settings: vec![],
        }
    }

    fn create(type_name: &str) -> Option<Box<dyn PluginNode>> {
        Op::from_type(type_name).map(|op| Box::new(MathNode(op)) as Box<dyn PluginNode>)
    }
}

export_plugin!(MathPlugin);
