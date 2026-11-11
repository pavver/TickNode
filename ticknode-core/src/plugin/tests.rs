    use super::*;
    use std::path::PathBuf;
    use crate::{Node, NodeContext, TickEngine};
    use super::host::domain_allowed;
    use super::outputs::validate_emission_outputs;
    use super::convert::{encode_value, from_wit_data_type};

    #[test]
    fn pushed_clear_validates_pin_kind_and_direction() {
        let def = crate::CompareNode::new(NodeId::generate()).definition();
        let (valid, errors) = validate_emission_outputs(&TypeRegistry::new(), &def, vec![wit::Output::Clear(0), wit::Output::Clear(2), wit::Output::Clear(99)]);
        assert_eq!(valid, vec![NodeOutput::Clear(PinId(2))]);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn domain_matching() {
        assert!(domain_allowed("example.com", "example.com"));
        assert!(domain_allowed("api.example.com", "example.com"));
        assert!(!domain_allowed("evilexample.com", "example.com"));
        assert!(domain_allowed("anything.net", "*"));
    }

    fn consumer_slot(runtime: Arc<Mutex<Runtime>>, pins: Vec<PinDefinition>, types: Vec<TypeDefinition>) -> PluginSlot {
        let definition = NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "consumer/node".into(),
            title: "Consumer".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            pins,
        };
        PluginSlot {
            manifest: PluginManifest::new("consumer", "0.1.0", "test"),
            definitions: vec![(definition, false)],
            types,
            runtime,
            active: true,
            settings_error: None,
            translations: Vec::new(),
        }
    }

    /// Disabling a plugin whose types are used by another enabled plugin (in pins or type fields)
    /// is rejected. The consumer is a mock: it shares its runtime with test_plugin because the check
    /// inspects only definitions.
    #[test]
    fn disabling_is_rejected_while_another_plugin_uses_its_types() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
        let Ok(bytes) = std::fs::read(&path) else {
            assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} not found", path.display());
            return;
        };
        let mut engine = TickEngine::new();
        engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
        engine.enable_plugin("test_plugin", &[]).unwrap();
        let runtime = engine.plugins["test_plugin"].runtime.clone();
        let point = DataType::Named("test.point".into());

        let pin = PinDefinition { id: PinId(0), name: "P".into(), pin_type: PinType::Data(DataType::List(Box::new(point.clone()))), direction: PinDirection::Input, required: false };
        engine.plugins.insert("consumer".into(), consumer_slot(runtime.clone(), vec![pin], vec![]));
        let err = engine.disable_plugin("test_plugin").unwrap_err();
        let err = err.to_string();
        assert!(err.contains("consumer") && err.contains("test_plugin"), "{err}");
        assert!(engine.plugin_active("test_plugin") && engine.types.get("test.point").is_some());

        let field = TypeDefinition::new_struct("consumer.wrap", "Wrap", vec![FieldDefinition::new("p", "P", point)]);
        engine.plugins.insert("consumer".into(), consumer_slot(runtime.clone(), vec![], vec![field]));
        assert!(engine.disable_plugin("test_plugin").is_err());

        // Consumer disabled — no dependency left
        engine.plugins.get_mut("consumer").unwrap().active = false;
        engine.disable_plugin("test_plugin").unwrap();
        assert!(engine.types.get("test.point").is_none());
    }

    fn data_emission(node: NodeId, pin: u32, value: DataValue) -> wit::Emission {
        wit::Emission { node: node.0.as_u64_pair(), outputs: vec![wit::Output::Data((pin, encode_value(&value)))] }
    }

    /// Emissions are applied only to existing nodes of the emitting plugin: a built-in node, a node of
    /// another plugin and an unknown id are dropped with an error log; invalid pins are dropped too.
    #[test]
    fn emissions_for_foreign_nodes_are_dropped() {
        let mut engine = TickEngine::new();
        let value = DataValue::Number(NumberValue::Int(7));
        // A built-in node (no owner) and a `const` node whose type is registered as owned by plugin "mine"
        let builtin = NodeId::generate();
        engine.create_node(builtin, "test/debug", &[]).unwrap();
        engine.registry.register_owned("mine", Box::new(|id| Box::new(crate::ConstNode { id, value: DataValue::Number(NumberValue::Int(0)), value_type: None })));
        let mine = NodeId::generate();
        engine.create_node(mine, "const", &[]).unwrap();
        let mut queue = std::collections::VecDeque::new();

        // Rejected for "other" (foreign plugin) and for "mine" when the node is built-in
        engine.apply_emissions("mine", vec![data_emission(builtin, 0, value.clone())], &mut queue);
        assert!(engine.data_cache.is_empty());
        engine.apply_emissions("other", vec![data_emission(mine, 0, value.clone())], &mut queue);
        assert!(engine.data_cache.is_empty());
        assert!(engine.host_state.logs.iter().any(|(n, m)| *n == mine && m.contains("not one of its nodes")), "{:?}", engine.host_state.logs);

        // Unknown node
        let ghost = NodeId::generate();
        engine.apply_emissions("mine", vec![data_emission(ghost, 0, value.clone())], &mut queue);
        assert!(!engine.data_cache.contains_key(&ghost));
        assert!(engine.host_state.logs.iter().any(|(n, _)| *n == ghost));

        // Invalid pins: a data value on a nonexistent pin and an exec pulse on a data pin
        engine.host_state.logs.clear();
        let bad = wit::Emission { node: mine.0.as_u64_pair(), outputs: vec![wit::Output::Exec(0), wit::Output::Data((99, encode_value(&value)))] };
        engine.apply_emissions("mine", vec![bad], &mut queue);
        assert_eq!(engine.host_state.logs.len(), 2, "{:?}", engine.host_state.logs);
        assert!(engine.data_cache.is_empty());

        // Valid emission is applied and traced like an execution
        engine.host_state.logs.clear();
        engine.apply_emissions("mine", vec![data_emission(mine, 0, value.clone())], &mut queue);
        assert_eq!(engine.data_cache[&mine][&PinId(0)], value);
        assert!(engine.executed.iter().any(|t| t.node_id == mine));
        assert!(engine.host_state.logs.is_empty());
    }

    /// Data-only plugin emissions remain available to pull consumers on later ticks.
    #[test]
    fn data_only_emissions_remain_pullable_across_ticks() {
        struct Live(NodeId);
        impl Node for Live {
            fn id(&self) -> NodeId { self.0 }
            fn definition(&self) -> NodeDefinition {
                NodeDefinition {
                    adaptive: false,
                    cost: 1,
                    type_name: "live".into(), title: "Live".into(), category: "Test".into(),
                    description: String::new(), help: String::new(), properties: Vec::new(),
                    pins: vec![PinDefinition { id: PinId(0), name: "Value".into(),
                        pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false }],
                }
            }
            fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
                panic!("an emission-driven data node must not execute on pull")
            }
        }
        let mut engine = TickEngine::new();
        engine.registry.register_owned("live_plugin", Box::new(|id| Box::new(Live(id))));
        let source = NodeId::generate();
        let sink = NodeId::generate();
        engine.create_node(source, "live", &[]).unwrap();
        engine.create_node(sink, "test/debug", &[]).unwrap();
        engine.connect_data(source, PinId(0), sink, PinId(1)).unwrap();
        let value = DataValue::from(21.5);
        let mut queue = std::collections::VecDeque::new();
        engine.apply_emissions("live_plugin", vec![data_emission(source, 0, value.clone())], &mut queue);
        assert!(queue.is_empty(), "data-only emissions do not schedule execution");
        for _ in 0..8 { engine.tick(); }
        engine.trigger_node(sink, PinId(0));
        engine.tick();
        let trace = engine.executed.iter().find(|t| t.node_id == sink).unwrap();
        assert!(trace.inputs.contains(&(PinId(1), Some(value))));
    }

    #[test]
    fn list_depth_becomes_nested_lists() {
        let t = from_wit_data_type(wit::DataType { base: wit::ScalarType::Number, list_depth: 2 });
        assert_eq!(t, DataType::List(Box::new(DataType::List(Box::new(DataType::Number)))));
    }

    #[test]
    fn plugin_emission_cost_reduces_queue_allowance() {
        let mut engine = TickEngine::new();
        engine.budget.limit = Some(2);
        engine.registry.register_owned("mine", Box::new(|id| Box::new(crate::ConstNode { id, value: 1i64.into(), value_type: None })));
        let source = NodeId::generate();
        engine.create_node(source, "const", &[]).unwrap();
        let ids: Vec<_> = (0..3).map(|_| {
            let id = NodeId::generate();
            engine.create_node(id, "test/click_button", &[]).unwrap();
            engine.trigger_node(id, PinId(0));
            id
        }).collect();
        let mut next = std::collections::VecDeque::new();
        engine.apply_emissions("mine", vec![data_emission(source, 0, 1i64.into())], &mut next);
        engine.run_queue(&mut next);
        assert_eq!(engine.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(), [source, ids[0]]);
        assert_eq!(engine.last_tick, crate::TickStats { spent: 2, deferred: 2 });
    }

mod core_c;
