use super::*;

#[test]
fn window_selection_requires_full_containment_and_includes_the_boundary() {
    let selection = SelectBox {
        start: (10.0, 20.0),
        end: (50.0, 60.0),
    };
    let view = Camera {
        x: 10.0,
        y: 20.0,
        zoom: 2.0,
    };
    assert!(selection.hits(view, Position { x: 0.0, y: 0.0 }, (20.0, 20.0)));
    for (x, y) in [(-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0)] {
        assert!(!selection.hits(view, Position { x, y }, (20.0, 20.0)));
    }
}

#[test]
fn crossing_selection_accepts_intersections_but_rejects_every_outside_edge() {
    let selection = SelectBox {
        start: (20.0, 20.0),
        end: (0.0, 0.0),
    };
    assert!(selection.crossing());
    for (x, y) in [(-5.0, 5.0), (20.0, 5.0), (5.0, -5.0), (5.0, 20.0)] {
        assert!(selection.hits(Camera::default(), Position { x, y }, (10.0, 10.0)));
    }
    for (x, y) in [(-11.0, 5.0), (21.0, 5.0), (5.0, -11.0), (5.0, 21.0)] {
        assert!(!selection.hits(Camera::default(), Position { x, y }, (10.0, 10.0)));
    }
}
