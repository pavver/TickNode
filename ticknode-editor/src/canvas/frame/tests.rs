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
