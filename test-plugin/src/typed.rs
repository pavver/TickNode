use ticknode_plugin_sdk::*;

pub(super) const POINT: &str = "test.point";
pub(super) const MODE: &str = "test.mode";

/// Emits a `test.point` with coordinates from properties.
pub(super) struct PointSource {
    pub(super) x: f64,
    pub(super) y: f64,
}

impl PluginNode for PointSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/point_source".into(),
            title: "Point Source".into(),
            category: "Test".into(),
            description: "Emits a test.point".into(),
            help: "On a signal at In emits a value of the custom type test.point on Point with coordinates X and Y (properties). The label field is not filled.".into(),
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
pub(super) struct BadSource;

impl PluginNode for BadSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/bad_source".into(),
            title: "Bad Source".into(),
            category: "Test".into(),
            description: "Emits a value of a foreign type".into(),
            help: "Host check: the output is declared as test.point, but the node emits an object of type other.type. The host must drop the value and log an error.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Point", DataType::Named(POINT.into()))],
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), StructValue::new("other.type").field("x", 1.0).build())]
    }
}

/// Accepts `test.point` and `test.mode`; outputs a formatted description and the sum of coordinates.
pub(super) struct PointSink;

impl PluginNode for PointSink {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/point_sink".into(),
            title: "Point Sink".into(),
            category: "Test".into(),
            description: "Accepts a point and a mode".into(),
            help: "Accepts values of types test.point and test.mode. Emits \"<mode>:<x>,<y>[:<label>]\" on Text and x + y on Sum.".into(),
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
