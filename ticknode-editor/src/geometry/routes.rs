//! Cubic wire routes and nearest-segment insertion, in world coordinates.

use ticknode_shared::Position;
use super::DU;

type Point = (f64, f64);

fn controls(a: Point, b: Point) -> (Point, Point) {
    let dx = ((b.0 - a.0).abs() / 2.0).max(10.0 * DU);
    ((a.0 + dx, a.1), (b.0 - dx, b.1))
}

/// A continuous route with horizontal tangents at each intermediate point.
pub fn route_path(a: Point, points: &[Position], b: Point) -> String {
    use std::fmt::Write;
    let mut path = format!("M {} {}", a.0, a.1);
    let mut start = a;
    for end in points.iter().map(|p| (p.x as f64, p.y as f64)).chain([b]) {
        let (c1, c2) = controls(start, end);
        write!(path, " C {} {}, {} {}, {} {}", c1.0, c1.1, c2.0, c2.1, end.0, end.1).unwrap();
        start = end;
    }
    path
}

fn cubic(a: Point, b: Point, t: f64) -> Point {
    let (c1, c2) = controls(a, b);
    let u = 1.0 - t;
    let axis = |a, c1, c2, b| u * u * u * a + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * b;
    (axis(a.0, c1.0, c2.0, b.0), axis(a.1, c1.1, c2.1, b.1))
}

fn distance_squared(p: Point, a: Point, b: Point) -> f64 {
    let d = (b.0 - a.0, b.1 - a.1);
    let length = d.0 * d.0 + d.1 * d.1;
    let t = if length == 0.0 { 0.0 } else { (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / length).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * d.0).powi(2) + (p.1 - a.1 - t * d.1).powi(2)
}

/// Insert before the endpoint of the nearest sampled cubic segment (24 samples each).
pub fn route_insert_index(a: Point, points: &[Position], b: Point, p: Point) -> usize {
    let mut start = a;
    let mut nearest = (f64::INFINITY, 0);
    for (index, end) in points.iter().map(|p| (p.x as f64, p.y as f64)).chain([b]).enumerate() {
        let mut previous = start;
        for sample in 1..=24 {
            let next = cubic(start, end, sample as f64 / 24.0);
            let distance = distance_squared(p, previous, next);
            if distance < nearest.0 { nearest = (distance, index); }
            previous = next;
        }
        start = end;
    }
    nearest.1
}

/// Halfway along the sampled route, including intermediate points and backwards curves.
pub fn route_midpoint(a: Point, points: &[Position], b: Point) -> Point {
    let mut samples = vec![a];
    let mut start = a;
    for end in points.iter().map(|p| (p.x as f64, p.y as f64)).chain([b]) {
        for n in 1..=24 { samples.push(cubic(start, end, n as f64 / 24.0)); }
        start = end;
    }
    let distance = |a: Point, b: Point| (b.0 - a.0).hypot(b.1 - a.1);
    let mut remaining = samples.windows(2).map(|p| distance(p[0], p[1])).sum::<f64>() / 2.0;
    for pair in samples.windows(2) {
        let length = distance(pair[0], pair[1]);
        if remaining <= length && length > 0.0 {
            let t = remaining / length;
            return (pair[0].0 + t * (pair[1].0 - pair[0].0), pair[0].1 + t * (pair[1].1 - pair[0].1));
        }
        remaining -= length;
    }
    a
}
