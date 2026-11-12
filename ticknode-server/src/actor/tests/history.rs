use super::*;

pub(super) fn actor(dir: &std::path::Path) -> Actor {
    test_actor(dir)
}

pub(super) fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    test_apply(actor, msg)
}

pub(super) fn add(actor: &mut Actor, x: f32) -> NodeId {
    apply(
        actor,
        ClientMessage::AddNode {
            type_name: "for_loop".into(),
            position: Position { x, y: 20.0 },
        },
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap()
}

pub(super) fn state(actor: &Actor, undo: bool, redo: bool) {
    let sheet = actor.sheets.first().map(|s| s.id);
    let ServerMessage::Init { history, .. } = actor.snapshot(sheet) else {
        panic!("missing Init")
    };
    assert_eq!(
        history,
        HistoryState {
            can_undo: undo,
            can_redo: redo
        }
    );
}

pub(super) fn actor_position(x: f32) -> Position {
    Position { x, y: 20.0 }
}

mod batches;
mod broadcast;
mod undo_redo;
