use super::*;

#[test]
fn arrows_move_by_a_grid_cell_alt_by_a_unit_and_shift_by_five_cells() {
    let at = |x: f64, y: f64| {
        Some(Position {
            x: x as f32,
            y: y as f32,
        })
    };
    assert_eq!(nudge_step("ArrowRight", false, false), at(GRID, 0.0));
    assert_eq!(nudge_step("ArrowLeft", false, false), at(-GRID, 0.0));
    assert_eq!(nudge_step("ArrowUp", true, false), at(0.0, -DU));
    assert_eq!(nudge_step("ArrowDown", false, true), at(0.0, 5.0 * GRID));
    assert_eq!(
        nudge_step("ArrowDown", true, true),
        at(0.0, DU),
        "Alt wins: the fine step"
    );
    assert_eq!(nudge_step("a", false, false), None);
    assert_eq!(nudge_step("Enter", true, true), None);
}

#[test]
fn a_burst_is_one_batch_of_absolute_positions() {
    let node = NodeId::generate();
    let commands = build_frame_move_batch(
        Vec::new(),
        vec![(node, Position { x: 20.0, y: 40.0 })],
        WireRoutes::new(),
        Position { x: -24.0, y: 0.0 },
    );
    assert_eq!(
        commands,
        vec![ticknode_shared::ClientMessage::MoveNode {
            id: node,
            position: Position { x: -4.0, y: 40.0 }
        }]
    );
}
