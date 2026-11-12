use crate::*;
use ticknode_shared::*;

mod costs;
mod nodes;

pub(super) fn add(
    engine: &mut TickEngine,
    name: &str,
    properties: &[(String, DataValue)],
) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

pub(super) fn property(name: &str, value: &str) -> (String, DataValue) {
    (name.into(), DataValue::from(value))
}

pub(super) fn number(value: i64) -> DataValue {
    DataValue::from(value)
}

pub(super) fn pull(engine: &mut TickEngine, id: NodeId, pin: u32) -> Option<DataValue> {
    engine.pull_data(id, PinId(pin), &mut Vec::new())
}

pub(super) fn activate(engine: &mut TickEngine, id: NodeId, pin: u32) -> NodeTrace {
    engine.trigger_node(id, PinId(pin));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|trace| trace.node_id == id)
        .unwrap()
        .clone()
}

pub(super) fn list(elements: &[i64]) -> DataValue {
    DataValue::List {
        element_type: DataType::Number,
        elements: elements.iter().copied().map(number).collect(),
    }
}

pub(super) fn object() -> DataValue {
    DataValue::Object {
        fields: [(
            "answer".into(),
            ObjectField {
                name: "answer".into(),
                description: String::new(),
                value: number(42),
            },
        )]
        .into(),
        type_name: None,
    }
}
