use ticknode_plugin_sdk::*;

pub(super) struct CostProbe { cost: u32 }
impl Default for CostProbe {
    fn default() -> Self { Self { cost: 1 } }
}
impl PluginNode for CostProbe {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            type_name: "test/cost".into(), title: "Cost Probe".into(), category: "Test".into(),
            description: "Exercises property-dependent work costs".into(),
            help: "Cost is relative work per call; native logic costs one unit. The cost property sets an arbitrary work cost.".into(),
            pins: vec![exec_in(0, "In")], properties: Vec::new(), cost: 1,
        }.cost(self.cost)
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("cost".into(), i64::from(self.cost).into())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if let ("cost", DataValue::Number(NumberValue::Int(cost))) = (name, value)
            && let Ok(cost) = u32::try_from(cost)
        {
            self.cost = cost;
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> { Vec::new() }
}
