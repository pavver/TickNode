use super::*;

#[test]
fn moving_nodes_snaps_each_coordinate_to_the_grid() {
    assert_eq!(moved_position(Position { x: 10.0, y: -10.0 }, (0.0, 0.0)), Position { x: 12.0, y: -12.0 });
    assert_eq!(moved_position(Position { x: 8.0, y: 12.0 }, (5.0, -3.0)), Position { x: 12.0, y: 8.0 });
}

#[test]
fn pinch_keeps_the_original_world_midpoint_under_the_new_midpoint() {
    let origin = Camera { x: 10.0, y: 20.0, zoom: 1.0 };
    let world = origin.to_world(50.0, 60.0);
    let camera = pinch_camera(10.0, (50.0, 60.0), origin, (60.0, 80.0), (80.0, 80.0));
    assert_eq!(camera.zoom, 2.0);
    assert_eq!(camera.to_world(70.0, 80.0), world);
    let clamped = pinch_camera(100.0, (50.0, 60.0), origin, (60.0, 80.0), (61.0, 80.0));
    assert_eq!(clamped.zoom, MIN_ZOOM);
    assert_eq!(clamped.to_world(60.5, 80.0), world);
}
