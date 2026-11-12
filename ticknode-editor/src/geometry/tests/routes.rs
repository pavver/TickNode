use super::*;

#[test]
fn routes_are_consecutive_cubics_and_empty_route_is_the_original_wire() {
    let a = (0.0, 0.0);
    let b = (400.0, 100.0);
    assert_eq!(route_path(a, &[], b), wire_path(a, b));
    assert_eq!(route_path(a, &[], b), "M 0 0 C 200 0, 200 100, 400 100");
    let points = [
        Position { x: 100.0, y: 80.0 },
        Position { x: 200.0, y: 40.0 },
    ];
    let path = route_path(a, &points, b);
    assert_eq!(path.matches('M').count(), 1);
    assert_eq!(path.matches('C').count(), 3);
    assert_eq!(
        path,
        "M 0 0 C 50 0, 50 80, 100 80 C 150 80, 150 40, 200 40 C 300 40, 300 100, 400 100"
    );
    assert_eq!(
        route_path((40.0, 80.0), &[], (0.0, 0.0)),
        "M 40 80 C 80 80, -40 0, 0 0"
    );
}

#[test]
fn insertion_uses_the_nearest_curve_segment_including_backwards_routes() {
    let points = [
        Position { x: 200.0, y: 0.0 },
        Position { x: 200.0, y: 200.0 },
    ];
    let a = (0.0, 0.0);
    let b = (400.0, 200.0);
    assert_eq!(route_insert_index(a, &points, b, (100.0, 0.0)), 0);
    assert_eq!(route_insert_index(a, &points, b, (200.0, 100.0)), 1);
    assert_eq!(route_insert_index(a, &points, b, (300.0, 200.0)), 2);
    assert_eq!(route_insert_index(a, &[], b, (-999.0, 999.0)), 0);
    // A right-to-left curve bows beyond its endpoints: insertion follows the curve.
    let points = [Position {
        x: -100.0,
        y: 100.0,
    }];
    assert_eq!(
        route_insert_index(a, &points, (100.0, 200.0), (-120.0, 98.0)),
        0
    );
    // Coincident endpoints do not divide by zero and tie-breaking is deterministic.
    assert_eq!(route_insert_index(a, &[Position::default()], a, a), 0);
}

#[test]
fn wire_label_midpoint_follows_routed_curves_and_handles_overlapping_pins() {
    let a = (0.0, 0.0);
    let b = (400.0, 100.0);
    let middle = route_midpoint(a, &[], b);
    assert!((middle.0 - 200.0).abs() < 0.01 && (middle.1 - 50.0).abs() < 0.01);
    let route = [Position { x: 200.0, y: 300.0 }];
    let middle = route_midpoint(a, &route, (400.0, 0.0));
    assert!((middle.0 - 200.0).abs() < 0.01 && (middle.1 - 300.0).abs() < 0.01);
    let same = route_midpoint(a, &[], a);
    assert!(same.0.is_finite() && same.1.is_finite());
}
