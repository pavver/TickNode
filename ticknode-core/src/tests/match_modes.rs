//! Match modes: first, all, and "only on change" (hysteresis).
use crate::builtin::MatchNode;
use crate::*;
use ticknode_shared::*;

fn list(items: &[&str]) -> DataValue {
    DataValue::List {
        element_type: DataType::String,
        elements: items.iter().map(|s| DataValue::from(*s)).collect(),
    }
}

fn hysteresis(engine: &mut TickEngine, extra: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    let mut properties = vec![
        ("inputs".to_string(), list(&["1:v:number"])),
        ("cases".to_string(), list(&["1: v > 230", "2: v < 200"])),
    ];
    properties.extend_from_slice(extra);
    engine.create_node(id, "match", &properties).unwrap();
    id
}

fn fire_with(engine: &mut TickEngine, id: NodeId, value: i64) -> Vec<PinId> {
    engine
        .set_input_value(id, PinId(1), Some(DataValue::from(value)))
        .unwrap();
    engine.trigger_node(id, PinId(0));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .map(|t| t.fired.clone())
        .unwrap_or_default()
}

fn mode_of(engine: &TickEngine, id: NodeId) -> Option<DataValue> {
    engine
        .node_model(id)
        .unwrap()
        .properties
        .into_iter()
        .find(|(n, _)| n == "mode")
        .map(|(_, v)| v)
}

#[test]
fn on_change_fires_each_case_once_and_keeps_it_through_the_dead_band() {
    let mut engine = TickEngine::new();
    let id = hysteresis(
        &mut engine,
        &[("mode".into(), DataValue::from("on_change"))],
    );
    let (on, off, none) = (vec![PinId(1)], vec![PinId(2)], Vec::<PinId>::new());
    let fired: Vec<_> = [240, 245, 220, 240, 190, 195, 220, 240]
        .into_iter()
        .map(|v| fire_with(&mut engine, id, v))
        .collect();
    let else_pin = vec![MatchNode::ELSE_PIN];
    assert_eq!(
        fired,
        [
            on.clone(),
            none.clone(),
            else_pin.clone(),
            none.clone(),
            off,
            none,
            else_pin,
            on
        ]
    );
}

#[test]
fn changing_cases_or_mode_resets_the_on_change_memory() {
    let mut engine = TickEngine::new();
    let id = hysteresis(
        &mut engine,
        &[("mode".into(), DataValue::from("on_change"))],
    );
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1)]);
    assert_eq!(fire_with(&mut engine, id, 240), []);
    engine
        .set_node_property(id, "cases", list(&["1: v > 231", "2: v < 200"]))
        .unwrap();
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1)]);
    engine
        .set_node_property(id, "mode", DataValue::from("on_change"))
        .unwrap();
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1)]);
}

#[test]
fn first_and_all_modes_fire_every_time() {
    let mut engine = TickEngine::new();
    let id = hysteresis(&mut engine, &[]);
    assert_eq!(mode_of(&engine, id), Some(DataValue::from("first")));
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1)]);
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1)]);
    assert_eq!(fire_with(&mut engine, id, 220), [MatchNode::ELSE_PIN]);
    assert_eq!(fire_with(&mut engine, id, 220), [MatchNode::ELSE_PIN]);
    engine
        .set_node_property(id, "cases", list(&["1: v > 230", "2: v > 0"]))
        .unwrap();
    engine
        .set_node_property(id, "mode", DataValue::from("all"))
        .unwrap();
    assert_eq!(fire_with(&mut engine, id, 240), [PinId(1), PinId(2)]);
}

#[test]
fn legacy_all_matches_loads_as_all_mode_and_unknown_modes_are_ignored() {
    let mut engine = TickEngine::new();
    let id = hysteresis(
        &mut engine,
        &[("all_matches".into(), DataValue::Bool(true))],
    );
    let properties = engine.node_model(id).unwrap().properties;
    assert_eq!(mode_of(&engine, id), Some(DataValue::from("all")));
    assert!(properties.iter().all(|(n, _)| n != "all_matches"));
    engine
        .set_node_property(id, "mode", DataValue::from("sometimes"))
        .unwrap();
    assert_eq!(mode_of(&engine, id), Some(DataValue::from("all")));
}

#[test]
fn quantity_constants_keep_their_units_and_compare_canonically() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node(
            id,
            "match",
            &[
                ("inputs".into(), list(&["1:t:quantity.temperature"])),
                (
                    "cases".into(),
                    list(&["1: t < 20 °C", "2: t < \"68 °F\"", "3: t >= 300 K"]),
                ),
                ("mode".into(), DataValue::from("all")),
            ],
        )
        .unwrap();
    // 19.5 °C is below 20 °C and below 68 °F (= 20 °C), but not above 300 K (26.85 °C).
    let fire = |engine: &mut TickEngine, celsius: f64| {
        engine
            .set_input_value(id, PinId(1), Some(DataValue::from(celsius)))
            .unwrap();
        engine.trigger_node(id, PinId(0));
        engine.tick();
        engine
            .executed
            .iter()
            .find(|t| t.node_id == id)
            .map(|t| t.fired.clone())
            .unwrap_or_default()
    };
    assert_eq!(fire(&mut engine, 19.5), vec![PinId(1), PinId(2)]);
    assert_eq!(fire(&mut engine, 27.0), vec![PinId(3)]);
    // The written units survive persistence.
    let cases = engine
        .node_model(id)
        .unwrap()
        .properties
        .into_iter()
        .find(|(n, _)| n == "cases")
        .unwrap()
        .1;
    let DataValue::List { elements, .. } = cases else {
        panic!("cases are a list")
    };
    assert!(
        elements
            .iter()
            .any(|c| matches!(c, DataValue::String(s) if s.contains("68 °F")))
    );
}
