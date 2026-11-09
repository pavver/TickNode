use ticknode_plugin_sdk::*;
use std::cell::RefCell;

/// Plugin settings: there is a single plugin instance, so settings live in static state.
#[derive(Default)]
pub(super) struct Config {
    pub(super) greeting: String,
    pub(super) mode: String,
    pub(super) token: String,
}

thread_local! {
    pub(super) static CONFIG: RefCell<Config> = RefCell::new(Config::default());
}

/// Emits the current greeting from settings: verifies that the host calls `configure`
/// (including after recreating the plugin instance).
pub(super) struct Greeter;

impl PluginNode for Greeter {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/greeter".into(),
            title: "Greeter".into(),
            category: "Test".into(),
            description: "Emits the greeting from settings".into(),
            help: "On a signal at In emits the current greeting from the plugin settings on Text as \"<greeting>|<mode>|<token set>\". Checks that the host applied the settings.".into(),
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

/// Node with described properties: a picker over the options the plugin publishes in `configure`
/// (source `greetings`) and a static drop-down.
pub(super) struct Picker {
    pub(super) choice: String,
    pub(super) mode: String,
}

impl PluginNode for Picker {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: vec![
                PropertyDefinition::dynamic("choice", "Choice", "greetings"),
                PropertyDefinition::select("mode", "Mode", &[("a", "Option A"), ("b", "Option B")]).described("Static options"),
            ],
            type_name: "test/picker".into(),
            title: "Picker".into(),
            category: "Test".into(),
            description: "Has described properties".into(),
            help: "Checks property schemas and dynamic property options. Emits \"<mode>:<choice>\" on Text.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Text", DataType::String)],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("choice".into(), self.choice.clone().into()), ("mode".into(), self.mode.clone().into())]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::String(s) = value {
            match name {
                "choice" => self.choice = s,
                "mode" => self.mode = s,
                _ => {}
            }
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), format!("{}:{}", self.mode, self.choice).into())]
    }
}

/// Publishes dynamic options derived from the greeting setting (checks `set-options` from `configure`,
/// the host limits and clearing on disable). `flood` exceeds the limits on purpose.
pub(super) fn publish_options(greeting: &str) {
    if greeting == "clock" {
        host::set_options("clock", &[PropertyOption { value: format!("{}:{}", host::current_tick(), host::tick_duration_ms()),
            label: "Clock outside execution".into(), detail: String::new(), icon: None }]);
    }
    let option = |value: &str| PropertyOption { value: value.into(), label: value.to_uppercase(), detail: format!("detail {value}"), icon: Some("mdi:hand-wave".into()) };
    host::set_options("greetings", &if greeting.is_empty() { Vec::new() } else { vec![option(greeting)] });
    if greeting == "flood" {
        let many: Vec<PropertyOption> = (0..20_005).map(|i| option(&format!("v{i}"))).collect();
        host::set_options("flood", &many);
        host::set_options("long", &[option(&"x".repeat(600)), option("ok"), PropertyOption { value: String::new(), ..option("empty") }]);
    } else {
        host::set_options("flood", &[]);
        host::set_options("long", &[]);
    }
}
