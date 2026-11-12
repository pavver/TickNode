//! Opaque identifier probes; names deliberately do not share the plugin's name.
use ticknode_plugin_sdk::*;
pub(super) const CHAT: &str = "opaque.chat";
pub(super) const USER: &str = "opaque.user";
pub(super) const ID: i64 = 9_007_199_254_740_993;

pub(super) fn publish(greeting: &str) {
    host::set_options(
        "ids",
        &[PropertyOption {
            value: ID.to_string(),
            label: format!("{greeting} chat"),
            detail: String::new(),
            icon: None,
        }],
    );
}

pub(super) struct Identifier {
    value: DataValue,
}
impl Default for Identifier {
    fn default() -> Self {
        Self { value: ID.into() }
    }
}
impl PluginNode for Identifier {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/identifier".into(),
            title: "Identifier probe".into(),
            category: "Test".into(),
            description: "Emits opaque integer identifiers".into(),
            help: "Copies Chat to the output, or emits the stored value to test output validation."
                .into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Chat", DataType::Named(CHAT.into())),
                data_out(2, "Chat", DataType::Named(CHAT.into())),
                data_out(3, "User", DataType::Named(USER.into())),
            ],
            ..Default::default()
        }
    }
    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let value = inputs
            .get(PinId(1))
            .cloned()
            .unwrap_or_else(|| self.value.clone());
        vec![
            Output::Data(PinId(2), value),
            Output::Data(PinId(3), self.value.clone()),
        ]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value" {
            self.value = value;
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("value".into(), self.value.clone())]
    }
}
