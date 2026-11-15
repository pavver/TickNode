use super::*;

struct RequiredProbe {
    id: NodeId,
    executed: bool,
}

impl Node for RequiredProbe {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/required_probe".into(),
            title: "Required probe".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            properties: vec![],
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "In".into(),
                    direction: PinDirection::Input,
                    pin_type: PinType::Exec,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Val".into(),
                    direction: PinDirection::Input,
                    pin_type: PinType::Data(DataType::Number),
                    required: true,
                },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if ctx.get_value(PinId(1)).is_some() {
            self.executed = true;
        }
        vec![]
    }
}

struct QuantitySink {
    id: NodeId,
    received: Option<DataValue>,
}

impl Node for QuantitySink {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/quantity_sink".into(),
            title: "Quantity sink".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            properties: vec![],
            pins: vec![PinDefinition {
                id: PinId(1),
                name: "Power".into(),
                direction: PinDirection::Input,
                pin_type: PinType::Data(DataType::Named("quantity.power".into())),
                required: false,
            }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        self.received = ctx.get_value(PinId(1));
        vec![]
    }
}

#[test]
fn sleep_delay_bound_to_var_timeout_waits_duration() {
    let mut engine = TickEngine::new();
    let sleep = NodeId::generate();
    engine.create_node(sleep, "sleep", &[]).unwrap();
    let sink = add_sink(&mut engine);
    engine
        .connect_exec(sleep, PinId(4), sink, PinId(0))
        .unwrap();

    engine.variables.create(
        VariableKey::global("timeout"),
        DataType::Duration,
        DataValue::Ticks(3),
    );
    engine
        .set_input_variable(sleep, PinId(1), Some(VariableRef::auto("timeout")))
        .unwrap();

    engine.trigger_node(sleep, PinId(0));
    engine.tick();
    assert_eq!(engine.delay_timers().len(), 1);
    assert_eq!(engine.delay_timers()[0].count, 1);

    // Ticks 1, 2, 3 of delay
    engine.tick();
    engine.tick();
    engine.tick();
    // Delivery queued sink; next tick executes it
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == sink));
}

#[test]
fn compare_and_math_add_bound_to_number_variable() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();

    engine.variables.create(
        VariableKey::global("a"),
        DataType::Number,
        DataValue::from(10),
    );
    engine.variables.create(
        VariableKey::global("b"),
        DataType::Number,
        DataValue::from(25),
    );

    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("a")))
        .unwrap();
    engine
        .set_input_variable(add, PinId(1), Some(VariableRef::auto("b")))
        .unwrap();

    let result = engine.pull_data(add, PinId(2), &mut vec![]);
    assert_eq!(result, Some(DataValue::from(35)));

    let cmp = NodeId::generate();
    engine.create_node(cmp, "compare", &[]).unwrap();
    engine.variables.create(
        VariableKey::global("target"),
        DataType::Number,
        DataValue::from(50),
    );
    engine
        .set_input_variable(cmp, PinId(1), Some(VariableRef::auto("target")))
        .unwrap();
    engine
        .set_input_value(cmp, PinId(0), Some(DataValue::from(50)))
        .unwrap();

    // Pin 2 is Eq
    let eq = engine.pull_data(cmp, PinId(2), &mut vec![]);
    assert_eq!(eq, Some(DataValue::Bool(true)));
}

#[test]
fn number_variable_bound_to_quantity_converts_like_wire() {
    let mut engine = TickEngine::new();
    let sink = NodeId::generate();
    engine.add_node(Box::new(QuantitySink {
        id: sink,
        received: None,
    }));

    engine.variables.create(
        VariableKey::global("p"),
        DataType::Number,
        DataValue::from(1500),
    );
    engine
        .set_input_variable(sink, PinId(1), Some(VariableRef::auto("p")))
        .unwrap();

    let mut stack = vec![];
    let inputs = engine.pull_inputs(sink, &mut stack);
    assert_eq!(inputs.get(&PinId(1)), Some(&Some(DataValue::from(1500))));
}

#[test]
fn string_variable_converts_to_number_and_incompatible_gives_no_value() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();

    engine.variables.create(
        VariableKey::global("s_num"),
        DataType::String,
        DataValue::String("5".into()),
    );
    engine.variables.create(
        VariableKey::global("incompatible"),
        DataType::List(Box::new(DataType::Number)),
        DataValue::List {
            element_type: DataType::Number,
            elements: vec![num(1)],
        },
    );

    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("s_num")))
        .unwrap();
    engine
        .set_input_value(add, PinId(1), Some(DataValue::from(10)))
        .unwrap();

    let result = engine.pull_data(add, PinId(2), &mut vec![]);
    assert_eq!(result, Some(DataValue::from(15)));

    engine.pure_cache.clear();
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("incompatible")))
        .unwrap();
    let result = engine.pull_data(add, PinId(2), &mut vec![]);
    // Incompatible gives None; math/add defaults unset A to 0, so 0 + 10 = 10
    assert_eq!(result, Some(DataValue::from(10)));
}

#[test]
fn sheet_vs_global_lookup_and_shadowing() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();
    let sheet = SheetId::generate();
    engine.set_node_sheet(add, Some(sheet));

    engine.variables.create(
        VariableKey::global("timeout"),
        DataType::Number,
        DataValue::from(10),
    );
    engine.variables.create(
        VariableKey::sheet(sheet, "timeout"),
        DataType::Number,
        DataValue::from(20),
    );

    // Auto lookup prefers sheet variable over global
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("timeout")))
        .unwrap();
    let res = engine.pull_data(add, PinId(2), &mut vec![]);
    assert_eq!(res, Some(DataValue::from(20)));

    // Global lookup skips sheet variable
    engine.pure_cache.clear();
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::global("timeout")))
        .unwrap();
    let res = engine.pull_data(add, PinId(2), &mut vec![]);
    assert_eq!(res, Some(DataValue::from(10)));
}

#[test]
fn missing_required_variable_blocks_and_reports_one_error_recovers_when_created() {
    let mut engine = TickEngine::new();
    let probe = NodeId::generate();
    engine.add_node(Box::new(RequiredProbe {
        id: probe,
        executed: false,
    }));

    engine
        .set_input_variable(probe, PinId(1), Some(VariableRef::auto("missing_var")))
        .unwrap();

    engine.trigger_node(probe, PinId(0));
    engine.tick();
    assert!(!engine.executed.iter().any(|t| t.node_id == probe));

    let errors = engine.node_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message.key, "error-node-required-variable");
    let pin_arg = errors[0]
        .message
        .args
        .iter()
        .find(|(k, _)| k == "pin")
        .map(|(_, v)| v.as_str());
    assert_eq!(pin_arg, Some("Val"));
    let var_arg = errors[0]
        .message
        .args
        .iter()
        .find(|(k, _)| k == "variable")
        .map(|(_, v)| v.as_str());
    assert_eq!(var_arg, Some("var.missing_var"));

    // Second tick: no duplicate error logged
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    assert_eq!(engine.node_errors().len(), 1);

    // Variable created: next execution succeeds and clears error
    engine.variables.create(
        VariableKey::global("missing_var"),
        DataType::Number,
        DataValue::from(42),
    );
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == probe));
    assert!(engine.node_errors().is_empty());
}

#[test]
fn binding_replaces_inline_value_and_vice_versa_dropped_when_pin_disappears_wire_precedence() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();

    // Inline value first
    engine.set_input_value(add, PinId(0), Some(num(5))).unwrap();
    assert!(
        engine
            .node_model(add)
            .unwrap()
            .properties
            .iter()
            .any(|(k, _)| k == "in:0")
    );

    // Setting variable removes inline value
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("my_var")))
        .unwrap();
    let model = engine.node_model(add).unwrap();
    assert!(!model.properties.iter().any(|(k, _)| k == "in:0"));
    assert!(
        model
            .properties
            .iter()
            .any(|(k, v)| k == "var:0" && *v == DataValue::String("var.my_var".into()))
    );

    // Setting inline value removes variable binding
    engine.set_input_value(add, PinId(0), Some(num(7))).unwrap();
    let model = engine.node_model(add).unwrap();
    assert!(model.properties.iter().any(|(k, _)| k == "in:0"));
    assert!(!model.properties.iter().any(|(k, _)| k == "var:0"));

    // Wire takes precedence over binding
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("my_var")))
        .unwrap();
    engine.variables.create(
        VariableKey::global("my_var"),
        DataType::Number,
        DataValue::from(100),
    );
    let src = NodeId::generate();
    engine
        .create_node(src, "const", &[("value".into(), num(999))])
        .unwrap();
    engine.connect_data(src, PinId(0), add, PinId(0)).unwrap();

    let res = engine.pull_data(add, PinId(2), &mut vec![]);
    // Wire source 999 takes precedence over variable binding 100
    assert_eq!(res, Some(DataValue::from(999)));

    // Disconnect wire: binding is active again
    engine.disconnect_data(add, PinId(0));
    engine.pure_cache.clear();
    let res = engine.pull_data(add, PinId(2), &mut vec![]);
    assert_eq!(res, Some(DataValue::from(100)));
}

#[test]
fn export_load_and_snippet_roundtrip_keep_var() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("x")))
        .unwrap();

    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_graph(&graph);
    assert_eq!(
        restored.input_variable(add, PinId(0)),
        Some(&VariableRef::auto("x"))
    );

    let snippet = engine.export_snippet(&[add]);
    let mut target = TickEngine::new();
    let (ids, _) = target.import_snippet(&snippet);
    assert_eq!(ids.len(), 1);
    assert_eq!(
        target.input_variable(ids[0].1, PinId(0)),
        Some(&VariableRef::auto("x"))
    );
}

#[test]
fn pure_node_recomputes_after_variable_set_in_graph_and_set_variable() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();

    engine.variables.create(
        VariableKey::global("val"),
        DataType::Number,
        DataValue::from(10),
    );
    engine
        .set_input_variable(add, PinId(0), Some(VariableRef::auto("val")))
        .unwrap();

    // Initial pull
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut vec![]),
        Some(DataValue::from(10))
    );
    assert!(engine.pure_cache.contains_key(&add));

    // Graph node variable/set
    let set_node = NodeId::generate();
    engine
        .create_node(
            set_node,
            "variable/set",
            &[
                ("name".into(), "val".into()),
                ("scope".into(), "global".into()),
            ],
        )
        .unwrap();
    engine
        .set_input_value(set_node, PinId(3), Some(DataValue::from(50)))
        .unwrap();
    engine.trigger_node(set_node, PinId(0));
    engine.tick();

    // Pure cache was cleared by variable/set; next pull recomputes with new value
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut vec![]),
        Some(DataValue::from(50))
    );

    // Direct variable update clears pure cache
    let key = VariableKey::global("val");
    engine.variables.set(&key, DataValue::from(100));
    engine.pure_cache.clear();
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut vec![]),
        Some(DataValue::from(100))
    );
}
