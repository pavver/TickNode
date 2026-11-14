use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, props: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, props).unwrap();
    id
}

fn activate(engine: &mut TickEngine, id: NodeId, pin: u32) -> NodeTrace {
    engine.trigger_node(id, PinId(pin));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone()
}

#[test]
fn set_and_reset_transitions_fire_only_on_change() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);

    // Set pulse turns on
    let trace = activate(&mut engine, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);

    // Second Set pulse keeps on — no Turned on transition
    let trace = activate(&mut engine, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // Reset pulse turns off
    let trace = activate(&mut engine, id, 1);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(2), PinId(3)]);

    // Second Reset pulse keeps off — no Turned off transition
    let trace = activate(&mut engine, id, 1);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(3)]);
}

#[test]
fn toggle_pulse_inverts_state() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);

    // Toggle from off -> on
    let trace = activate(&mut engine, id, 2);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);

    // Toggle from on -> off
    let trace = activate(&mut engine, id, 2);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(2), PinId(3)]);

    // Toggle from off -> on again
    let trace = activate(&mut engine, id, 2);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);
}

#[test]
fn initial_state_on() {
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[("initial".into(), "on".into())],
    );

    // Initial Set does not fire Turned on because initial state is already on
    let trace = activate(&mut engine, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // Reset turns off
    let trace = activate(&mut engine, id, 1);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(2), PinId(3)]);
}

#[test]
fn level_inputs_and_evaluate() {
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);

    // Initially S=false, R=false: Evaluate does not change state
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // Level S=true: Evaluate turns on
    engine
        .set_input_value(id, PinId(3), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);

    // S=true again: no transition
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // S=false, R=true: Evaluate turns off
    engine
        .set_input_value(id, PinId(3), Some(DataValue::Bool(false)))
        .unwrap();
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(2), PinId(3)]);

    // S=false, R=false: latch keeps previous off state
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(false)))
        .unwrap();
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(3)]);
}

#[test]
fn priority_level_inputs_both_ways() {
    // Priority reset (default): S=true and R=true -> off
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);
    engine
        .set_input_value(id, PinId(3), Some(DataValue::Bool(true)))
        .unwrap();
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // Priority set: S=true and R=true -> on
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[("priority".into(), "set".into())],
    );
    engine
        .set_input_value(id, PinId(3), Some(DataValue::Bool(true)))
        .unwrap();
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 5);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);
}

#[test]
fn priority_exec_pulses_in_same_tick() {
    // Priority reset (default): Set then Reset in same tick -> off
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);
    engine.trigger_node(id, PinId(0));
    engine.trigger_node(id, PinId(1));
    engine.tick();
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone();
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);

    // Priority reset (default): Reset then Set in same tick -> off
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);
    engine.trigger_node(id, PinId(1));
    engine.trigger_node(id, PinId(0));
    engine.tick();
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone();
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);

    // Priority set: Reset then Set in same tick -> on
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[("priority".into(), "set".into())],
    );
    engine.trigger_node(id, PinId(1));
    engine.trigger_node(id, PinId(0));
    engine.tick();
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone();
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);

    // Priority set: Set then Reset in same tick -> on
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[("priority".into(), "set".into())],
    );
    engine.trigger_node(id, PinId(0));
    engine.trigger_node(id, PinId(1));
    engine.tick();
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone();
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
}

#[test]
fn level_and_pulse_interaction() {
    // Interlock: level R=true, Set pulse arrives. Priority reset -> stays off.
    let mut engine = TickEngine::new();
    let id = add(&mut engine, "logic/rs_trigger", &[]);
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(false))]);
    assert_eq!(trace.fired, vec![PinId(3)]);

    // Interlock with priority set: Set pulse wins -> turns on.
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[("priority".into(), "set".into())],
    );
    engine
        .set_input_value(id, PinId(4), Some(DataValue::Bool(true)))
        .unwrap();
    let trace = activate(&mut engine, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(1), PinId(3)]);
}

#[test]
fn persistence_of_properties() {
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/rs_trigger",
        &[
            ("priority".into(), "set".into()),
            ("initial".into(), "on".into()),
        ],
    );
    let graph = engine.export_graph();

    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    let def = restored.node_definition(id).unwrap();
    assert_eq!(def.type_name, "logic/rs_trigger");
    assert_eq!(def.cost, 1);

    // Initial state is on after restore
    let trace = activate(&mut restored, id, 0);
    assert_eq!(trace.outputs, vec![(PinId(0), DataValue::Bool(true))]);
    assert_eq!(trace.fired, vec![PinId(3)]); // No Turned on transition because already on
}
