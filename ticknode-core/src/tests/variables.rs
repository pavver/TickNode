use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, properties: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

fn property(name: &str, value: &str) -> (String, DataValue) {
    (name.into(), DataValue::from(value))
}

fn number(value: i64) -> DataValue {
    DataValue::from(value)
}

fn pull(engine: &mut TickEngine, id: NodeId, pin: u32) -> Option<DataValue> {
    engine.pull_data(id, PinId(pin), &mut Vec::new())
}

fn activate(engine: &mut TickEngine, id: NodeId, pin: u32) -> NodeTrace {
    engine.trigger_node(id, PinId(pin));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|trace| trace.node_id == id)
        .cloned()
        .unwrap_or_else(|| panic!("node {id:?} did not execute"))
}

#[test]
fn test_variable_get_and_set() {
    let mut engine = TickEngine::new();
    let key = VariableKey::global("speed");
    assert!(engine.variables.create(key.clone(), DataType::Number, number(10)));

    let get = add(&mut engine, "variable/get", &[property("name", "speed")]);
    assert_eq!(pull(&mut engine, get, 0), Some(number(10)));
    assert_eq!(pull(&mut engine, get, 1), Some(DataValue::Bool(true)));

    let set = add(&mut engine, "variable/set", &[property("name", "speed")]);
    engine.set_input_value(set, PinId(3), Some(number(25))).unwrap();

    let trace = activate(&mut engine, set, 0);
    assert_eq!(trace.fired, vec![PinId(1)]); // Pin 1 = Out
    assert_eq!(engine.variables.get(&key).map(|v| &v.value), Some(&number(25)));
    assert_eq!(pull(&mut engine, get, 0), Some(number(25)));
}

#[test]
fn test_variable_set_type_conversion_and_failure() {
    let mut engine = TickEngine::new();
    let key = VariableKey::global("temp");
    assert!(engine.variables.create(key.clone(), DataType::Number, number(0)));

    let set = add(&mut engine, "variable/set", &[property("name", "temp")]);

    // Compatible/convertible string "42" -> converted to Number 42
    engine.set_input_value(set, PinId(3), Some(DataValue::from("42"))).unwrap();
    let trace = activate(&mut engine, set, 0);
    assert_eq!(trace.fired, vec![PinId(1)]);
    assert_eq!(engine.variables.get(&key).map(|v| &v.value), Some(&number(42)));

    // Inconvertible string "hello" -> failure, fires Failed (Pin 2)
    engine.set_input_value(set, PinId(3), Some(DataValue::from("hello"))).unwrap();
    let trace = activate(&mut engine, set, 0);
    assert_eq!(trace.fired, vec![PinId(2)]);
    assert!(engine.node_errors().iter().any(|e| e.node == set));
}

#[test]
fn test_variable_set_missing_variable() {
    let mut engine = TickEngine::new();
    let set = add(&mut engine, "variable/set", &[property("name", "nonexistent")]);
    engine.set_input_value(set, PinId(3), Some(number(100))).unwrap();

    let trace = activate(&mut engine, set, 0);
    assert_eq!(trace.fired, vec![PinId(2)]); // Pin 2 = Failed
    assert!(engine.node_errors().iter().any(|e| e.node == set));
}

#[test]
fn test_variable_create_and_delete() {
    let mut engine = TickEngine::new();
    let create = add(
        &mut engine,
        "variable/create",
        &[property("name", "counter"), property("scope", "global"), property("type", "number")],
    );
    engine.set_input_value(create, PinId(3), Some(number(1))).unwrap();

    // First create: Created (Pin 1)
    let trace = activate(&mut engine, create, 0);
    assert_eq!(trace.fired, vec![PinId(1)]);
    let key = VariableKey::global("counter");
    assert_eq!(engine.variables.get(&key).map(|v| &v.value), Some(&number(1)));

    // Second create with same name: Exists (Pin 2), does not overwrite
    engine.set_input_value(create, PinId(3), Some(number(999))).unwrap();
    let trace = activate(&mut engine, create, 0);
    assert_eq!(trace.fired, vec![PinId(2)]);
    assert_eq!(engine.variables.get(&key).map(|v| &v.value), Some(&number(1)));

    // Delete node
    let delete = add(&mut engine, "variable/delete", &[property("name", "counter")]);
    let trace = activate(&mut engine, delete, 0);
    assert_eq!(trace.fired, vec![PinId(1)]);
    assert!(engine.variables.get(&key).is_none());

    let get = add(&mut engine, "variable/get", &[property("name", "counter")]);
    assert_eq!(pull(&mut engine, get, 0), None);
    assert_eq!(pull(&mut engine, get, 1), Some(DataValue::Bool(false)));
}

#[test]
fn test_variable_scope_shadowing_and_lookup() {
    let mut engine = TickEngine::new();
    let sheet_id = SheetId::generate();
    let sheet_key = VariableKey::sheet(sheet_id, "mode");
    let global_key = VariableKey::global("mode");

    assert!(engine.variables.create(sheet_key, DataType::String, DataValue::from("sheet_val")));
    assert!(engine.variables.create(global_key, DataType::String, DataValue::from("global_val")));

    // Node on sheet_id with Auto lookup -> finds sheet variable
    let get_auto = add(&mut engine, "variable/get", &[property("name", "mode"), property("lookup", "auto")]);
    engine.set_node_sheet(get_auto, Some(sheet_id));
    assert_eq!(pull(&mut engine, get_auto, 0), Some(DataValue::from("sheet_val")));

    // Node on sheet_id with Global lookup -> finds global variable
    let get_global = add(&mut engine, "variable/get", &[property("name", "mode"), property("lookup", "global")]);
    engine.set_node_sheet(get_global, Some(sheet_id));
    assert_eq!(pull(&mut engine, get_global, 0), Some(DataValue::from("global_val")));

    // Node without sheet (or on another sheet) with Auto lookup -> finds global variable
    let other_sheet = SheetId::generate();
    let get_other = add(&mut engine, "variable/get", &[property("name", "mode"), property("lookup", "auto")]);
    engine.set_node_sheet(get_other, Some(other_sheet));
    assert_eq!(pull(&mut engine, get_other, 0), Some(DataValue::from("global_val")));
}

#[test]
fn test_variable_changed_fires_next_tick() {
    let mut engine = TickEngine::new();
    let key = VariableKey::global("target");
    assert!(engine.variables.create(key.clone(), DataType::Number, number(0)));

    let changed = add(&mut engine, "variable/changed", &[property("name", "target")]);
    let sink = add(&mut engine, "console", &[]);
    engine.connect_exec(changed, PinId(0), sink, PinId(0)).unwrap();
    engine.connect_data(changed, PinId(1), sink, PinId(1)).unwrap();

    // Tick 1: no changes yet
    engine.tick();
    assert_eq!(engine.host_state.logs.len(), 0);

    // Tick 2: set variable target
    let set = add(&mut engine, "variable/set", &[property("name", "target")]);
    engine.set_input_value(set, PinId(3), Some(number(50))).unwrap();
    activate(&mut engine, set, 0);

    // Tick 3: changed polls and fires, queuing sink
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == changed));

    // Tick 4: sink executes and logs the value
    engine.tick();
    assert_eq!(engine.host_state.logs, vec![(sink, "50".into())]);
    engine.host_state.logs.clear();

    // Tick 5: no changes, neither should fire
    engine.tick();
    assert_eq!(engine.host_state.logs.len(), 0);
}

#[test]
fn test_legacy_var_name_property_migration() {
    let mut engine = TickEngine::new();
    let key = VariableKey::global("migrated");
    assert!(engine.variables.create(key, DataType::Number, number(77)));

    let get = add(&mut engine, "variable/get", &[property("var_name", "migrated")]);
    assert_eq!(pull(&mut engine, get, 0), Some(number(77)));

    // Verify property name was migrated to "name"
    let model = engine.node_model(get).unwrap();
    assert!(model.properties.iter().any(|(k, v)| k == "name" && v == &DataValue::from("migrated")));
}
