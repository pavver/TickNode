use super::*;
use ticknode_core::{Node, NodeContext};

struct Probe {
    id: NodeId,
    duration: DataValue,
}
impl Node for Probe {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/property".into(),
            title: "Property".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            cost: 1,
            adaptive: false,
            pins: vec![],
            properties: vec![PropertyDefinition::duration("duration", "Duration", true)],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("duration".into(), self.duration.clone())]
    }
    fn set_property(&mut self, _: &str, value: DataValue) {
        self.duration = value;
    }
    fn validate_property(&self, _: &str, value: &DataValue) -> Result<(), UiMessage> {
        if *value == DataValue::from("60s") {
            Err(UiMessage::new("test-property-rejected", "Rejected"))
        } else {
            Ok(())
        }
    }
}

fn actor(dir: &std::path::Path) -> Actor {
    let mut actor = test_actor(dir);
    actor.engine.registry.register(Box::new(|id| {
        Box::new(Probe {
            id,
            duration: "1s".into(),
        })
    }));
    actor
}

#[test]
fn rejected_commit_replies_with_stored_node_without_history_or_broadcast() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let events = test_apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "test/property".into(),
            position: Position::default(),
        },
    );
    let id = events
        .into_iter()
        .find_map(|e| match e {
            ServerMessage::NodeAdded(n) => Some(n.model.id),
            _ => None,
        })
        .unwrap();
    let sheet = actor.sheets[0].id;
    let before = actor.graph_snapshot();
    let history = actor.histories[&sheet].undo.len();
    for (value, key) in [
        ("abc", "error-property-duration"),
        ("60s", "test-property-rejected"),
    ] {
        let (reply, mut receive) = mpsc::unbounded_channel();
        let error = actor
            .apply(
                ClientMessage::SetProperty {
                    id,
                    name: "duration".into(),
                    value: value.into(),
                },
                Some(sheet),
                &reply,
            )
            .unwrap_err();
        assert_eq!(error.key, key);
        let ServerMessage::NodeUpdated(node) = receive.try_recv().unwrap() else {
            panic!("missing rollback reply")
        };
        assert_eq!(node.model.properties, before.nodes[0].properties);
        assert!(receive.try_recv().is_err());
        assert_eq!(actor.graph_snapshot(), before);
        assert_eq!(actor.histories[&sheet].undo.len(), history);
    }
}

#[test]
fn add_with_validates_but_settings_paste_and_history_restore_stored_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet = actor.sheets[0].id;
    for value in ["abc", "60s"] {
        let (reply, mut receive) = mpsc::unbounded_channel();
        assert!(
            actor
                .apply(
                    ClientMessage::AddNodeWith {
                        type_name: "test/property".into(),
                        position: Position::default(),
                        properties: vec![("duration".into(), value.into())],
                    },
                    Some(sheet),
                    &reply
                )
                .is_err()
        );
        assert!(actor.engine.node_ids().is_empty());
        assert!(receive.try_recv().is_err());
        assert!(
            actor
                .histories
                .get(&sheet)
                .is_none_or(|h| h.undo.is_empty())
        );
    }
    let events = test_apply(
        &mut actor,
        ClientMessage::AddNodeWith {
            type_name: "test/property".into(),
            position: Position::default(),
            properties: vec![("duration".into(), "2s".into())],
        },
    );
    let id = events
        .into_iter()
        .find_map(|e| match e {
            ServerMessage::NodeAdded(n) => Some(n.model.id),
            _ => None,
        })
        .unwrap();
    let accepted = actor.graph_snapshot();
    test_apply(
        &mut actor,
        ClientMessage::PasteSettings {
            id,
            type_name: "test/property".into(),
            properties: vec![("duration".into(), "60s".into())],
        },
    );
    let pasted = actor.graph_snapshot();
    assert_eq!(
        pasted.nodes[0].properties,
        [("duration".into(), "60s".into())]
    );
    test_apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), accepted);
    test_apply(&mut actor, ClientMessage::Redo);
    assert_eq!(actor.graph_snapshot(), pasted);
}
