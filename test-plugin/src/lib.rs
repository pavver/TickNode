//! Plugin for host integration tests (`ticknode-core/tests/plugins.rs`).

use std::cell::RefCell;

use ticknode_plugin_sdk::*;

struct TestPlugin;

/// Plugin settings: there is a single plugin instance, so settings live in static state.
#[derive(Default)]
struct Config {
    greeting: String,
    mode: String,
    token: String,
}

thread_local! {
    static CONFIG: RefCell<Config> = RefCell::new(Config::default());
}

/// Emits the current greeting from settings: verifies that the host calls `configure`
/// (including after recreating the plugin instance).
struct Greeter;

impl PluginNode for Greeter {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/greeter".into(),
            title: "Greeter".into(),
            category: "Test".into(),
            description: "Видає привітання з налаштувань".into(),
            help: "За сигналом на In видає на Text поточне привітання з налаштувань плагіна у вигляді «<привітання>|<режим>|<токен задано>». Перевіряє, що хост застосував налаштування.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Text", DataType::String)],
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        let text = CONFIG.with(|c| {
            let c = c.borrow();
            format!("{}|{}|{}", c.greeting, c.mode, if c.token.is_empty() { "no-token" } else { "token" })
        });
        vec![Output::Data(PinId(1), text.into())]
    }
}

/// Pulse counter: state is held in a node field, step is a property.
struct Counter {
    step: i64,
    count: i64,
}

impl PluginNode for Counter {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/counter".into(),
            title: "Counter".into(),
            category: "Test".into(),
            description: "Рахує імпульси".into(),
            help: "Кожен сигнал на In збільшує лічильник Count на 1. Перевіряє, що стан ноди плагіна зберігається між викликами.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Count", DataType::Number)],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("step".into(), DataValue::Number(NumberValue::Int(self.step)))]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let ("step", DataValue::Number(n)) = (name, value) {
            self.step = n.to_f64() as i64;
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        self.count += self.step;
        vec![Output::Data(PinId(1), DataValue::Number(NumberValue::Int(self.count)))]
    }

    /// Not an event source: the host must not call `poll`.
    fn poll(&mut self, _: u64) -> Option<PinId> {
        panic!("poll для ноди без polls")
    }
}

/// Event source: triggers on every `poll` and outputs the poll count on Polls.
struct Ticker {
    polls: i64,
}

impl PluginNode for Ticker {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/ticker".into(),
            title: "Ticker".into(),
            category: "Test".into(),
            description: "Спрацьовує щотакту".into(),
            help: "Джерело подій: хост опитує ноду щотакту, і вона щоразу видає сигнал на Out і кількість опитувань на Polls.".into(),
            pins: vec![exec_out(0, "Out"), data_out(1, "Polls", DataType::Number)],
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), DataValue::Number(NumberValue::Int(self.polls))), Output::Exec(PinId(0))]
    }

    fn poll(&mut self, _: u64) -> Option<PinId> {
        self.polls += 1;
        Some(PinId(0))
    }

    fn polls(&self) -> bool { true }
}

/// `panic` — panics, `spin` — infinite loop (must be terminated by the fuel limit).
struct Crash;

impl PluginNode for Crash {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/crash".into(),
            title: "Crash".into(),
            category: "Test".into(),
            description: "Аварійно завершує плагін".into(),
            help: "Перевірка ізоляції плагінів: сигнал на Panic викликає паніку, на Spin — нескінченний цикл. Хост має перервати виконання й перезапустити плагін.".into(),
            pins: vec![exec_in(0, "Panic"), exec_in(1, "Spin")],
        }
    }

    fn execute(&mut self, trigger: Trigger, _: &Inputs) -> Vec<Output> {
        match trigger {
            Trigger::Pin(PinId(0)) => panic!("навмисна паніка"),
            Trigger::Pin(PinId(1)) => {
                let mut x: u64 = 0;
                loop {
                    x = std::hint::black_box(x.wrapping_add(1));
                }
            }
            _ => Vec::new(),
        }
    }
}

/// Writes the input string to a file and reads it back (`Storage` capability).
struct Storage;

impl PluginNode for Storage {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/storage".into(),
            title: "Storage".into(),
            category: "Test".into(),
            description: "Перевіряє файлове сховище плагіна".into(),
            help: "За сигналом на In записує Text у файл у власному каталозі плагіна й видає на Read прочитаний назад вміст. Перевіряє право Storage.".into(),
            pins: vec![exec_in(0, "In"), data_in(1, "Text", DataType::String), data_out(2, "Read", DataType::String)],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let result = std::fs::write("note.txt", inputs.string(PinId(1)))
            .and_then(|()| std::fs::read_to_string("note.txt"))
            .unwrap_or_else(|e| format!("error: {e}"));
        vec![Output::Data(PinId(2), DataValue::String(result))]
    }
}

const POINT: &str = "test.point";
const MODE: &str = "test.mode";

/// Emits a `test.point` with coordinates from properties.
struct PointSource {
    x: f64,
    y: f64,
}

impl PluginNode for PointSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/point_source".into(),
            title: "Point Source".into(),
            category: "Test".into(),
            description: "Видає точку test.point".into(),
            help: "За сигналом на In видає на Point значення власного типу test.point з координатами X і Y (властивості). Поле label не заповнюється.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Point", DataType::Named(POINT.into()))],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("x".into(), self.x.into()), ("y".into(), self.y.into())]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::Number(n) = value {
            match name {
                "x" => self.x = n.to_f64(),
                "y" => self.y = n.to_f64(),
                _ => {}
            }
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), StructValue::new(POINT).field("x", self.x).field("y", self.y).build())]
    }
}

/// Contract violator: declares output as `test.point`, but emits an object of a foreign type.
struct BadSource;

impl PluginNode for BadSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/bad_source".into(),
            title: "Bad Source".into(),
            category: "Test".into(),
            description: "Видає значення чужого типу".into(),
            help: "Перевірка хоста: вихід оголошено як test.point, але нода видає об'єкт типу other.type. Хост має відкинути значення й записати помилку в лог.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Point", DataType::Named(POINT.into()))],
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), StructValue::new("other.type").field("x", 1.0).build())]
    }
}

/// Accepts `test.point` and `test.mode`; outputs a formatted description and the sum of coordinates.
struct PointSink;

impl PluginNode for PointSink {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/point_sink".into(),
            title: "Point Sink".into(),
            category: "Test".into(),
            description: "Приймає точку й режим".into(),
            help: "Приймає значення типів test.point і test.mode. На Text видає «<режим>:<x>,<y>[:<label>]», на Sum — x + y.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Point", DataType::Named(POINT.into())).required(),
                data_in(2, "Mode", DataType::Named(MODE.into())),
                data_out(3, "Text", DataType::String),
                data_out(4, "Sum", DataType::Number),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let (Some(x), Some(y)) = (inputs.field_number(PinId(1), "x"), inputs.field_number(PinId(1), "y")) else {
            return Vec::new();
        };
        let mut text = format!("{}:{x},{y}", inputs.string(PinId(2)));
        if let Some(label) = inputs.field_string(PinId(1), "label") {
            text.push_str(&format!(":{label}"));
        }
        vec![Output::Data(PinId(3), text.into()), Output::Data(PinId(4), (x + y).into())]
    }
}

impl Plugin for TestPlugin {
    const NODE_TYPES: &'static [&'static str] =
        &["test/bad_source", "test/counter", "test/crash", "test/greeter", "test/point_sink", "test/point_source", "test/storage", "test/ticker"];

    fn configure(settings: &[(String, DataValue)]) -> Result<(), String> {
        let token = setting_str(settings, "token").unwrap_or_default().to_string();
        CONFIG.with(|c| {
            let mut c = c.borrow_mut();
            c.greeting = setting_str(settings, "greeting").unwrap_or_default().to_string();
            c.mode = setting_str(settings, "mode").unwrap_or("quiet").to_string();
            c.token = token.clone();
        });
        match token.as_str() {
            "" => Err("token не задано".into()),
            "bad" => Err("невірний токен".into()),
            _ => Ok(()),
        }
    }

    fn types() -> Vec<TypeDefinition> {
        vec![
            TypeDefinition::new_struct(POINT, "Point", vec![
                FieldDefinition::new("x", "X", DataType::Number),
                FieldDefinition::new("y", "Y", DataType::Number),
                FieldDefinition::new("label", "Label", DataType::String).optional(),
            ]).described("Точка на площині").colored("#e0a030"),
            TypeDefinition::new_enum(MODE, "Mode", &[("auto", "Авто"), ("manual", "Вручну")]),
        ]
    }

    fn manifest() -> PluginManifest {
        PluginManifest::new("test_plugin", "0.1.0", "TickNode")
            .capability(Capability::Storage)
            .setting(SettingDefinition::text("greeting", "Привітання").described("Що видає нода Greeter").default_value("Hello"))
            .setting(SettingDefinition::secret("token", "Токен").described("Порожній чи «bad» плагін відхиляє").required())
            .setting(SettingDefinition::select("mode", "Режим", &[("quiet", "Тихо"), ("loud", "Гучно")]).default_value("quiet"))
    }

    fn create(type_name: &str) -> Option<Box<dyn PluginNode>> {
        Some(match type_name {
            "test/counter" => Box::new(Counter { step: 1, count: 0 }),
            "test/crash" => Box::new(Crash),
            "test/greeter" => Box::new(Greeter),
            "test/bad_source" => Box::new(BadSource),
            "test/point_source" => Box::new(PointSource { x: 0.0, y: 0.0 }),
            "test/point_sink" => Box::new(PointSink),
            "test/storage" => Box::new(Storage),
            "test/ticker" => Box::new(Ticker { polls: 0 }),
            _ => return None,
        })
    }
}

export_plugin!(TestPlugin);
