//! Cubic wire routes and nearest-segment insertion, in world coordinates.

use super::DU;
use std::fmt::Write;
use ticknode_shared::Position;

pub type Point = (f64, f64);

/// Collects endpoints and intermediate positions into an ordered route.
pub fn route_points(a: Point, points: &[Position], b: Point) -> Vec<Point> {
    let mut route = Vec::with_capacity(points.len() + 2);
    route.push(a);
    route.extend(points.iter().map(|p| (p.x as f64, p.y as f64)));
    route.push(b);
    route
}

/// Normalized direction tangent at route point `index`.
///
/// Intermediate points use a Catmull-Rom direction from the previous to the next point.
/// When neighbours coincide, a perpendicular to the incoming line ensures a smooth U-turn.
fn route_tangent(route: &[Point], index: usize) -> Point {
    if index == 0 || index + 1 >= route.len() {
        return (1.0, 0.0);
    }
    let prev = route[index - 1];
    let curr = route[index];
    let next = route[index + 1];
    let delta = (next.0 - prev.0, next.1 - prev.1);
    let len = delta.0.hypot(delta.1);
    if len > 1e-6 {
        (delta.0 / len, delta.1 / len)
    } else {
        let v = (curr.0 - prev.0, curr.1 - prev.1);
        let v_len = v.0.hypot(v.1);
        if v_len > 1e-6 {
            (-v.1 / v_len, v.0 / v_len)
        } else {
            (1.0, 0.0)
        }
    }
}

/// Yields the control points `(c1, c2)` for segment `i` of `route`.
///
/// - The real start endpoint leaves heading right (+x).
/// - The real end endpoint enters heading right (from -x).
/// - Intermediate route points follow the route direction (Catmull-Rom tangent)
///   with handle lengths proportional to the adjacent segment length (one third),
///   ensuring smooth U-turns without loops and maintaining tangent continuity.
pub fn segment_controls(route: &[Point], i: usize) -> (Point, Point) {
    let n = route.len();
    if n < 2 || i + 1 >= n {
        let fallback = if let Some(&pt) = route.last() {
            pt
        } else {
            (0.0, 0.0)
        };
        return (fallback, fallback);
    }
    let start = route[i];
    let end = route[i + 1];
    let seg_len = (end.0 - start.0).hypot(end.1 - start.1);

    let c1 = if i == 0 {
        let dx = ((end.0 - start.0).abs() / 2.0).max(10.0 * DU);
        (start.0 + dx, start.1)
    } else {
        let tangent = route_tangent(route, i);
        let handle_len = seg_len / 3.0;
        (
            start.0 + tangent.0 * handle_len,
            start.1 + tangent.1 * handle_len,
        )
    };

    let c2 = if i + 1 == n - 1 {
        let dx = ((end.0 - start.0).abs() / 2.0).max(10.0 * DU);
        (end.0 - dx, end.1)
    } else {
        let tangent = route_tangent(route, i + 1);
        let handle_len = seg_len / 3.0;
        (
            end.0 - tangent.0 * handle_len,
            end.1 - tangent.1 * handle_len,
        )
    };

    (c1, c2)
}

/// A continuous route with horizontal tangents at endpoints and direction-aware tangents
/// at intermediate route points.
pub fn route_path(a: Point, points: &[Position], b: Point) -> String {
    let route = route_points(a, points, b);
    let mut path = format!("M {} {}", a.0, a.1);
    for i in 0..route.len() - 1 {
        let (c1, c2) = segment_controls(&route, i);
        let end = route[i + 1];
        let _ = write!(
            path,
            " C {} {}, {} {}, {} {}",
            c1.0, c1.1, c2.0, c2.1, end.0, end.1
        );
    }
    path
}

fn cubic(start: Point, c1: Point, c2: Point, end: Point, t: f64) -> Point {
    let u = 1.0 - t;
    let axis =
        |a, c1, c2, b| u * u * u * a + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * b;
    (
        axis(start.0, c1.0, c2.0, end.0),
        axis(start.1, c1.1, c2.1, end.1),
    )
}

fn distance_squared(p: Point, a: Point, b: Point) -> f64 {
    let d = (b.0 - a.0, b.1 - a.1);
    let length = d.0 * d.0 + d.1 * d.1;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / length).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * d.0).powi(2) + (p.1 - a.1 - t * d.1).powi(2)
}

/// Insert before the endpoint of the nearest sampled cubic segment (24 samples each).
pub fn route_insert_index(a: Point, points: &[Position], b: Point, p: Point) -> usize {
    let route = route_points(a, points, b);
    let mut nearest = (f64::INFINITY, 0);
    for i in 0..route.len() - 1 {
        let start = route[i];
        let end = route[i + 1];
        let (c1, c2) = segment_controls(&route, i);
        let mut previous = start;
        for sample in 1..=24 {
            let next = cubic(start, c1, c2, end, sample as f64 / 24.0);
            let distance = distance_squared(p, previous, next);
            if distance < nearest.0 {
                nearest = (distance, i);
            }
            previous = next;
        }
    }
    nearest.1
}

/// Halfway along the sampled route, including intermediate points and backwards curves.
pub fn route_midpoint(a: Point, points: &[Position], b: Point) -> Point {
    let route = route_points(a, points, b);
    let mut samples = vec![a];
    for i in 0..route.len() - 1 {
        let start = route[i];
        let end = route[i + 1];
        let (c1, c2) = segment_controls(&route, i);
        for n in 1..=24 {
            samples.push(cubic(start, c1, c2, end, n as f64 / 24.0));
        }
    }
    let distance = |a: Point, b: Point| (b.0 - a.0).hypot(b.1 - a.1);
    let mut remaining = samples
        .windows(2)
        .map(|p| distance(p[0], p[1]))
        .sum::<f64>()
        / 2.0;
    for pair in samples.windows(2) {
        let length = distance(pair[0], pair[1]);
        if remaining <= length && length > 0.0 {
            let t = remaining / length;
            return (
                pair[0].0 + t * (pair[1].0 - pair[0].0),
                pair[0].1 + t * (pair[1].1 - pair[0].1),
            );
        }
        remaining -= length;
    }
    a
}
