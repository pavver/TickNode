use crate::*;
use ticknode_shared::*;

pub(super) fn add(e: &mut TickEngine, name: &str) -> NodeId {
    let id = NodeId::generate();
    e.create_node(id, name, &[]).unwrap();
    id
}

pub(super) fn pin(id: u32, direction: PinDirection, ty: DataType) -> PinDefinition {
    PinDefinition {
        id: PinId(id),
        name: format!("Value {id}"),
        direction,
        pin_type: PinType::Data(ty),
        required: false,
    }
}

// A native fixture with deliberately overlapping input/output ids, as supported by the graph model.
pub(super) struct FixedNode {
    pub(super) id: NodeId,
    pub(super) pins: Vec<PinDefinition>,
    pub(super) outputs: Vec<NodeOutput>,
}

impl Node for FixedNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/fixed".into(),
            title: "Fixed".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            properties: vec![],
            pins: self.pins.clone(),
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        self.outputs.clone()
    }
}

pub(super) fn fixed(
    e: &mut TickEngine,
    pins: Vec<PinDefinition>,
    outputs: Vec<NodeOutput>,
) -> NodeId {
    e.registry.register(Box::new(move |id| {
        Box::new(FixedNode {
            id,
            pins: pins.clone(),
            outputs: outputs.clone(),
        })
    }));
    add(e, "test/fixed")
}

pub(super) fn point_type(e: &mut TickEngine) {
    e.register_types(
        &[TypeDefinition::new_struct(
            "test.point",
            "Point",
            vec![FieldDefinition::new("x", "X", DataType::Number)],
        )],
        &[],
    )
    .unwrap();
}

pub(super) fn point(x: i64) -> DataValue {
    DataValue::Object {
        type_name: Some("test.point".into()),
        fields: [(
            "x".into(),
            ObjectField {
                name: "x".into(),
                description: String::new(),
                value: x.into(),
            },
        )]
        .into(),
    }
}

mod execution;
mod graph;
mod stubs;
