use super::*;
use crate::i18n::{with_locale, Locale};

#[test]
fn frame_geometry_without_selection_centers_default_size() {
    let center = (501.2, 303.8);
    let (pos, size) = calculate_new_frame_geometry(None, center);

    assert_eq!(size.width, 400.0);
    assert_eq!(size.height, 240.0);

    // 501.2 - 200 = 301.2 -> snapped to DU (4.0) -> 300.0
    assert_eq!(pos.x, snap_du_f32(501.2 - 200.0));
    // 303.8 - 120 = 183.8 -> snapped to DU (4.0) -> 184.0
    assert_eq!(pos.y, snap_du_f32(303.8 - 120.0));
}

#[test]
fn frame_geometry_with_selection_adds_correct_padding_and_snaps() {
    // Single node from (100.0, 100.0) to (300.0, 250.0)
    let bounds = Some((100.0, 100.0, 300.0, 250.0));
    let (pos, size) = calculate_new_frame_geometry(bounds, (0.0, 0.0));

    // Padding sides and bottom = 6 * DU = 24.0
    // Padding top = 6 * DU + header (6 * DU) = 48.0
    assert_eq!(pos.x, 100.0 - 24.0); // 76.0
    assert_eq!(pos.y, 100.0 - 48.0); // 52.0

    // Width = (300 - 100) + 2 * 24.0 = 248.0
    assert_eq!(size.width, 248.0);
    // Height = (250 - 100) + 24.0 + 48.0 = 222.0 -> snapped to DU = 224.0
    assert_eq!(size.height, 224.0);
}

#[test]
fn frame_geometry_clamps_to_min_size() {
    // Very tiny bounding box (1x1)
    let bounds = Some((100.0, 100.0, 101.0, 101.0));
    let (_pos, size) = calculate_new_frame_geometry(bounds, (0.0, 0.0));

    assert!(size.width >= MIN_FRAME_W);
    assert!(size.height >= MIN_FRAME_H);
}

#[test]
fn frame_color_classes_and_titles() {
    for (color, expected_class) in [
        (FrameColor::Gray, "frame-gray"),
        (FrameColor::Red, "frame-red"),
        (FrameColor::Orange, "frame-orange"),
        (FrameColor::Yellow, "frame-yellow"),
        (FrameColor::Green, "frame-green"),
        (FrameColor::Teal, "frame-teal"),
        (FrameColor::Blue, "frame-blue"),
        (FrameColor::Purple, "frame-purple"),
    ] {
        assert_eq!(frame_color_class(color), expected_class);
    }

    with_locale(Locale::En, || {
        assert_eq!(frame_color_title(FrameColor::Gray), "Gray");
        assert_eq!(frame_color_title(FrameColor::Red), "Red");
        assert_eq!(frame_color_title(FrameColor::Orange), "Orange");
        assert_eq!(frame_color_title(FrameColor::Yellow), "Yellow");
        assert_eq!(frame_color_title(FrameColor::Green), "Green");
        assert_eq!(frame_color_title(FrameColor::Teal), "Teal");
        assert_eq!(frame_color_title(FrameColor::Blue), "Blue");
        assert_eq!(frame_color_title(FrameColor::Purple), "Purple");
    });

    with_locale(Locale::Uk, || {
        assert_eq!(frame_color_title(FrameColor::Gray), "Сірий");
        assert_eq!(frame_color_title(FrameColor::Red), "Червоний");
        assert_eq!(frame_color_title(FrameColor::Orange), "Помаранчевий");
        assert_eq!(frame_color_title(FrameColor::Yellow), "Жовтий");
        assert_eq!(frame_color_title(FrameColor::Green), "Зелений");
        assert_eq!(frame_color_title(FrameColor::Teal), "Бірюзовий");
        assert_eq!(frame_color_title(FrameColor::Blue), "Синій");
        assert_eq!(frame_color_title(FrameColor::Purple), "Фіолетовий");
    });
}

#[test]
fn rect_contains_requires_full_containment() {
    let container = (100.0, 100.0, 200.0, 200.0);
    // Fully inside
    assert!(rect_contains(container, (110.0, 110.0, 50.0, 50.0)));
    // Exactly matching boundary
    assert!(rect_contains(container, (100.0, 100.0, 200.0, 200.0)));
    // Touching edges
    assert!(rect_contains(container, (100.0, 100.0, 50.0, 50.0)));
    assert!(rect_contains(container, (250.0, 250.0, 50.0, 50.0)));

    // Partially sticking out left
    assert!(!rect_contains(container, (90.0, 110.0, 50.0, 50.0)));
    // Partially sticking out right
    assert!(!rect_contains(container, (260.0, 110.0, 50.0, 50.0)));
    // Partially sticking out top
    assert!(!rect_contains(container, (110.0, 90.0, 50.0, 50.0)));
    // Partially sticking out bottom
    assert!(!rect_contains(container, (110.0, 260.0, 50.0, 50.0)));
    // Completely outside
    assert!(!rect_contains(container, (400.0, 400.0, 50.0, 50.0)));
    // Larger than container
    assert!(!rect_contains(container, (50.0, 50.0, 300.0, 300.0)));
}

#[test]
fn nodes_and_frames_inside_filters_accurately() {
    let container = (0.0, 0.0, 400.0, 300.0);
    let n1 = NodeId::generate();
    let n2 = NodeId::generate();
    let n3 = NodeId::generate();

    let nodes = [
        (n1, Position { x: 50.0, y: 50.0 }, 100.0, 80.0), // inside
        (n2, Position { x: 350.0, y: 50.0 }, 100.0, 80.0), // sticks out right
        (n3, Position { x: 200.0, y: 150.0 }, 150.0, 100.0), // inside
    ];
    let inside_nodes = nodes_fully_inside(container, nodes);
    assert_eq!(inside_nodes, vec![n1, n3]);

    let f_self = FrameId::generate();
    let f_inner = FrameId::generate();
    let f_outer = FrameId::generate();
    let frames = [
        (f_self, Position { x: 0.0, y: 0.0 }, NodeSize { width: 400.0, height: 300.0 }),
        (f_inner, Position { x: 20.0, y: 20.0 }, NodeSize { width: 200.0, height: 150.0 }),
        (f_outer, Position { x: 300.0, y: 200.0 }, NodeSize { width: 200.0, height: 150.0 }),
    ];
    let inside_frames = frames_fully_inside(container, Some(f_self), frames);
    assert_eq!(inside_frames, vec![f_inner]);
}

#[test]
fn frame_resize_snaps_and_clamps() {
    let origin = NodeSize { width: 200.0, height: 160.0 };
    // Snaps to DU (4.0)
    assert_eq!(
        resize_frame_size(origin, (7.0, 9.0)),
        NodeSize { width: 208.0, height: 168.0 }
    );
    // Clamps to min
    assert_eq!(
        resize_frame_size(origin, (-9999.0, -9999.0)),
        NodeSize { width: MIN_FRAME_W, height: MIN_FRAME_H }
    );
    // Clamps to max
    assert_eq!(
        resize_frame_size(origin, (99999.0, 99999.0)),
        NodeSize { width: MAX_FRAME_SIZE, height: MAX_FRAME_SIZE }
    );
}

#[test]
fn build_frame_move_batch_translates_frames_nodes_and_routes() {
    let f_id = FrameId::generate();
    let frame = Frame {
        id: f_id,
        title: "Test".into(),
        color: FrameColor::Blue,
        position: Position { x: 100.0, y: 80.0 },
        size: NodeSize { width: 300.0, height: 200.0 },
    };
    let n_id = NodeId::generate();
    let node_pos = Position { x: 120.0, y: 100.0 };

    let wire_key = (true, ConnectionModel {
        src_node: n_id,
        src_pin: PinId(0),
        dst_node: n_id,
        dst_pin: PinId(1),
    });
    let routes = [(wire_key, vec![Position { x: 150.0, y: 150.0 }])];

    let delta = Position { x: 40.0, y: -20.0 };
    let batch = build_frame_move_batch(vec![frame], vec![(n_id, node_pos)], routes, delta);
    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch[0],
        ClientMessage::SetFrame(Frame {
            id: f_id,
            title: "Test".into(),
            color: FrameColor::Blue,
            position: Position { x: 140.0, y: 60.0 },
            size: NodeSize { width: 300.0, height: 200.0 },
        })
    );
    assert_eq!(
        batch[1],
        ClientMessage::MoveNode {
            id: n_id,
            position: Position { x: 160.0, y: 80.0 },
        }
    );
    assert_eq!(
        batch[2],
        ClientMessage::SetWireRoute(WireRoute {
            exec: true,
            connection: wire_key.1,
            points: vec![Position { x: 190.0, y: 130.0 }],
        })
    );

    // Zero delta produces empty batch
    let empty = build_frame_move_batch(vec![], vec![], vec![], Position { x: 0.0, y: 0.0 });
    assert!(empty.is_empty());
}
