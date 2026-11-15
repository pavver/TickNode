use ticknode_plugin_sdk::*;

/// Pulse counter: state is held in a node field, step is a property.
pub(super) struct Counter {
    pub(super) step: i64,
    pub(super) count: i64,
}

impl PluginNode for Counter {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/counter".into(),
            title: "Counter".into(),
            category: "Test".into(),
            description: "Counts pulses".into(),
            help: "Each signal on In increases the counter Count by 1. Checks that the plugin node state is kept between calls.".into(),
            pins: vec![exec_in(0, "In"), data_out(1, "Count", DataType::Number)],
            ..Default::default()
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![(
            "step".into(),
            DataValue::Number(NumberValue::Int(self.step)),
        )]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let ("step", DataValue::Number(n)) = (name, value) {
            self.step = n.to_f64() as i64;
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        self.count += self.step;
        vec![Output::Data(
            PinId(1),
            DataValue::Number(NumberValue::Int(self.count)),
        )]
    }

    /// Not an event source: the host must not call `poll`.
    fn poll(&mut self, _: u64) -> Option<PinId> {
        panic!("poll called for a node without polls")
    }
}

/// Event source: triggers on every `poll` and outputs the poll count on Polls.
pub(super) struct Ticker {
    pub(super) polls: i64,
}

impl PluginNode for Ticker {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/ticker".into(),
            title: "Ticker".into(),
            category: "Test".into(),
            description: "Fires every tick".into(),
            help: "Event source: the host polls the node every tick, and each time it emits a signal on Out and the number of polls on Polls.".into(),
            pins: vec![exec_out(0, "Out"), data_out(1, "Polls", DataType::Number)],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![
            Output::Data(PinId(1), DataValue::Number(NumberValue::Int(self.polls))),
            Output::Exec(PinId(0)),
        ]
    }

    fn poll(&mut self, _: u64) -> Option<PinId> {
        self.polls += 1;
        Some(PinId(0))
    }

    fn polls(&self) -> bool {
        true
    }
}

/// `panic` — panics, `spin` — infinite loop (must be terminated by the fuel limit).
pub(super) struct Crash;

impl PluginNode for Crash {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/crash".into(),
            title: "Crash".into(),
            category: "Test".into(),
            description: "Crashes the plugin".into(),
            help: "Plugin isolation check: a signal on Panic causes a panic, on Spin an infinite loop. The host must interrupt execution and restart the plugin.".into(),
            pins: vec![exec_in(0, "Panic"), exec_in(1, "Spin")],
            ..Default::default()
        }
    }

    fn execute(&mut self, trigger: Trigger, _: &Inputs) -> Vec<Output> {
        match trigger {
            Trigger::Pin(PinId(0)) => panic!("deliberate panic"),
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
pub(super) struct Storage;

impl PluginNode for Storage {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/storage".into(),
            title: "Storage".into(),
            category: "Test".into(),
            description: "Checks the plugin file storage".into(),
            help: "On a signal at In writes Text to a file in the plugin's own directory and emits the content read back on Read. Checks the Storage permission.".into(),
            pins: vec![exec_in(0, "In"), data_in(1, "Text", DataType::String), data_out(2, "Read", DataType::String)],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let result = std::fs::write("note.txt", inputs.string(PinId(1)))
            .and_then(|()| std::fs::read_to_string("note.txt"))
            .unwrap_or_else(|e| format!("error: {e}"));
        vec![Output::Data(PinId(2), DataValue::String(result))]
    }
}

/// Exercises clearing valid data outputs and rejecting clears of other pins.
pub(super) struct ClearProbe;
impl PluginNode for ClearProbe {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/clear".into(),
            title: "Clear".into(),
            category: "Test".into(),
            description: "Sets or clears a data output".into(),
            help: "Set emits a value; Clear removes it and attempts invalid clears.".into(),
            properties: vec![],
            pins: vec![
                exec_in(0, "Set"),
                data_out(1, "Value", DataType::Number),
                exec_in(2, "Clear"),
                data_in(3, "Input", DataType::Number),
                exec_out(4, "Out"),
            ],
            ..Default::default()
        }
    }
    fn execute(&mut self, trigger: Trigger, _: &Inputs) -> Vec<Output> {
        if matches!(trigger, Trigger::Pin(PinId(0))) {
            vec![Output::Data(PinId(1), 42i64.into())]
        } else {
            vec![
                Output::Clear(PinId(1)),
                Output::Clear(PinId(0)),
                Output::Clear(PinId(3)),
                Output::Clear(PinId(4)),
                Output::Clear(PinId(99)),
            ]
        }
    }
}

/// Pure test node: doubles input on Out (0) and exposes its execution counter on Calls (2).
#[derive(Default)]
pub(super) struct PureDouble {
    pub(super) calls: i64,
}

impl PluginNode for PureDouble {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test/pure_double", "Pure Double", "Test")
            .description("Pure node that doubles its input")
            .help(
                "Pure test node: evaluates on demand when output is pulled and caches the result.",
            )
            .pins(vec![
                data_in(1, "In", DataType::Number),
                data_out(0, "Out", DataType::Number),
                data_out(2, "Calls", DataType::Number),
            ])
            .pure()
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        self.calls += 1;
        let mut outputs = vec![Output::Data(
            PinId(2),
            DataValue::Number(NumberValue::Int(self.calls)),
        )];
        if let Some(DataValue::Number(n)) = inputs.get(PinId(1)) {
            outputs.push(Output::Data(
                PinId(0),
                DataValue::Number(NumberValue::Float(n.to_f64() * 2.0)),
            ));
        }
        outputs
    }
}

/// Invalid pure node definition with an exec pin: must be treated as non-pure with a warning.
pub(super) struct PureInvalid;

impl PluginNode for PureInvalid {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test/pure_invalid", "Pure Invalid", "Test")
            .description("Invalid pure definition that has an exec pin")
            .help("Host check: a definition with pure=true and exec pins must be treated as non-pure with a warning.")
            .pins(vec![
                exec_in(0, "In"),
                data_out(1, "Out", DataType::Number),
            ])
            .pure()
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![Output::Data(PinId(1), 100i64.into())]
    }
}

/// Pure node that emits an exec output: must be ignored by the host with an English warning.
pub(super) struct PureExecPulse;

impl PluginNode for PureExecPulse {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test/pure_exec_pulse", "Pure Exec Pulse", "Test")
            .description("Pure node that emits an execution pulse")
            .help("Host check: Output::Execution from a pure node must be ignored with a throttled English warning.")
            .pins(vec![
                data_out(0, "Out", DataType::Number),
            ])
            .pure()
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![
            Output::Data(PinId(0), 42i64.into()),
            Output::Exec(PinId(99)),
        ]
    }
}
