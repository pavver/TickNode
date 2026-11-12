use ticknode_plugin_sdk::*;

/// Exposes host clock values and deliberately advertises a reserved pin for host validation.
pub(super) struct Clock;

impl PluginNode for Clock {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            type_name: "test/clock".into(),
            title: "Host Clock".into(),
            category: "Test".into(),
            description: "Reports the host clock".into(),
            help: "Checks the call context and reserved pin filtering in the plugin host.".into(),
            cost: 1,
            properties: vec![],
            pins: vec![
                exec_in(0, "In"),
                data_out(1, "Tick", DataType::Number),
                data_out(2, "Duration", DataType::Number),
                data_out(0x8000_0000, "Reserved", DataType::Number),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![
            Output::Data(PinId(1), (host::current_tick() as i64).into()),
            Output::Data(PinId(2), i64::from(host::tick_duration_ms()).into()),
        ]
    }
}
