use super::routes::{route_points, segment_controls};
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
        route_path((40.0, 80.0), &[], (0.0, 0.0)),
        "M 40 80 C 80 80, -40 0, 0 0"
    );
}

#[test]
fn back_turning_route_produces_smooth_u_turn_with_tangent_continuity() {
    // Output at (300, 50), turning back to the left to (100, 100), then into input at (300, 150).
    // Both neighbours of p1 are to its right (x = 300).
    let a = (300.0, 50.0);
    let p1 = Position { x: 100.0, y: 100.0 };
    let b = (300.0, 150.0);
    let pts = route_points(a, &[p1], b);

    let (c1_0, c2_0) = segment_controls(&pts, 0);
    let (c1_1, c2_1) = segment_controls(&pts, 1);

    // Endpoints retain horizontal convention with minimum handle 10 * DU (40 px):
    assert_eq!(c1_0.1, a.1);
    assert!(c1_0.0 > a.0);
    assert_eq!(c2_1.1, b.1);
    assert!(c2_1.0 < b.0);

    // At the intermediate point p1, both neighbours are at x = 300, so tangent is vertical:
    let p = (p1.x as f64, p1.y as f64);
    assert!((c2_0.0 - p.0).abs() < 1e-6);
    assert!((c1_1.0 - p.0).abs() < 1e-6);

    // Control points at p1 do not point away from direction of travel (heading down along +y):
    assert!(c2_0.1 < p.1);
    assert!(c1_1.1 > p.1);

    // Tangent continuity: incoming handle (c2_0 -> p) and outgoing handle (p -> c1_1)
    // are collinear and point in the same travel direction.
    let arriving = (p.0 - c2_0.0, p.1 - c2_0.1);
    let leaving = (c1_1.0 - p.0, c1_1.1 - p.1);
    let cross = arriving.0 * leaving.1 - arriving.1 * leaving.0;
    assert!(cross.abs() < 1e-6);
    let dot = arriving.0 * leaving.0 + arriving.1 * leaving.1;
    assert!(dot > 0.0);

    // The handles (c2_0 - p) and (c1_1 - p) are opposite vectors relative to p:
    let h_in = (c2_0.0 - p.0, c2_0.1 - p.1);
    let h_out = (c1_1.0 - p.0, c1_1.1 - p.1);
    let cos_angle = (h_in.0 * h_out.0 + h_in.1 * h_out.1) / (arriving.1 * leaving.1);
    assert!((cos_angle - -1.0).abs() < 1e-6);
}

#[test]
fn multi_point_route_tangent_continuity_at_every_intermediate_point() {
    let a = (0.0, 0.0);
    let points = [
        Position { x: 100.0, y: 150.0 },
        Position { x: 50.0, y: 300.0 },
        Position { x: 250.0, y: 200.0 },
    ];
    let b = (400.0, 100.0);
    let pts = route_points(a, &points, b);

    for i in 1..pts.len() - 1 {
        let p = pts[i];
        let (_, c2_prev) = segment_controls(&pts, i - 1);
        let (c1_next, _) = segment_controls(&pts, i);

        let v_in = (p.0 - c2_prev.0, p.1 - c2_prev.1);
        let v_out = (c1_next.0 - p.0, c1_next.1 - p.1);

        let cross = v_in.0 * v_out.1 - v_in.1 * v_out.0;
        assert!(
            cross.abs() < 1e-6,
            "Intermediate point {i} must have collinear tangents"
        );
        let dot = v_in.0 * v_out.0 + v_in.1 * v_out.1;
        assert!(
            dot > 0.0,
            "Tangents at point {i} must point along direction of travel"
        );
    }
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

    // Back-turning U-turn produces a valid midpoint on the route:
    let u_turn = [Position { x: 100.0, y: 100.0 }];
    let apex_mid = route_midpoint((300.0, 50.0), &u_turn, (300.0, 150.0));
    assert!(apex_mid.0 >= 100.0 && apex_mid.0 <= 400.0);
    assert!(apex_mid.1 >= 50.0 && apex_mid.1 <= 150.0);
    assert!(apex_mid.0.is_finite() && apex_mid.1.is_finite());
}
