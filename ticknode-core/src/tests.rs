mod behavior;
mod builtin_contracts;
mod convert;
mod definitions;
mod engine_execution;
mod hysteresis;
mod legacy_nodes;
mod match_execution;
mod match_modes;
mod math_quantities;
mod mutation_contracts;
mod property_input;
mod property_validation;
mod quantities;
mod variables;

use super::*;
pub(crate) use ticknode_shared::*;

pub(crate) fn cases(list: &[&str]) -> (String, DataValue) {
    (
        "cases".to_string(),
        DataValue::List {
            element_type: DataType::String,
            elements: list
                .iter()
                .map(|c| DataValue::String(c.to_string()))
                .collect(),
        },
    )
}

pub(crate) fn num(v: i64) -> DataValue {
    DataValue::Number(NumberValue::Int(v))
}

/// Sink node with exec input PinId(0) for tracing execution flow.
pub(crate) fn add_sink(engine: &mut TickEngine) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, "test/debug", &[]).unwrap();
    id
}

pub(crate) fn fire(engine: &mut TickEngine, id: NodeId) -> Vec<PinId> {
    engine.trigger_node(id, PinId(0));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .map(|t| t.fired.clone())
        .unwrap_or_default()
}
