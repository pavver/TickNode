use super::*;

fn option(value: &str, label: &str) -> PropertyOption {
    PropertyOption {
        value: value.into(),
        label: label.into(),
        detail: value.into(),
        icon: None,
    }
}

#[test]
fn mdi_icons_become_font_classes() {
    assert_eq!(
        mdi_class("mdi:lightbulb").as_deref(),
        Some("mdi mdi-lightbulb")
    );
    assert_eq!(
        mdi_class("mdi:door-open").as_deref(),
        Some("mdi mdi-door-open")
    );
    assert_eq!(
        mdi_class("mdi:battery-10").as_deref(),
        Some("mdi mdi-battery-10")
    );
    // Other icon sets and unsafe names have no glyph (and never reach the class attribute)
    assert_eq!(mdi_class("phu:bulb"), None);
    assert_eq!(mdi_class("mdi:"), None);
    assert_eq!(mdi_class("mdi:a b\" onclick=x"), None);
    assert_eq!(mdi_class("lightbulb"), None);
}

#[test]
fn search_matches_label_detail_and_value_case_insensitively() {
    let options = vec![
        option("light.kitchen", "Kitchen Ceiling"),
        option("light.hall", "Hall"),
        option("sensor.kitchen_temp", "Temperature"),
    ];
    assert_eq!(filter_options(&options, "", 10), (vec![0, 1, 2], 3));
    assert_eq!(filter_options(&options, "KITCHEN", 10), (vec![0, 2], 2));
    // Every term must match somewhere (label or id)
    assert_eq!(filter_options(&options, "kitchen ceil", 10), (vec![0], 1));
    assert_eq!(filter_options(&options, "sensor temp", 10), (vec![2], 1));
    assert_eq!(filter_options(&options, "nothing", 10), (vec![], 0));
}

#[test]
fn search_caps_the_rendered_rows_but_counts_all_matches() {
    let options: Vec<PropertyOption> = (0..2500)
        .map(|i| option(&format!("sensor.s{i}"), &format!("Sensor {i}")))
        .collect();
    let (shown, total) = filter_options(&options, "", MAX_ROWS);
    assert_eq!((shown.len(), total), (MAX_ROWS, 2500));
    let (shown, total) = filter_options(&options, "sensor 12", MAX_ROWS);
    assert!(shown.len() <= MAX_ROWS && total >= shown.len());
    assert!(shown.iter().all(|i| options[*i].label.contains("12")));
}

#[test]
fn popup_opens_below_or_above_and_stays_on_screen() {
    // Plenty of room below
    let p = placement((100.0, 100.0, 220.0, 124.0), (1280.0, 800.0));
    assert!(!p.up && p.edge == 124.0 && p.left == 100.0);
    assert_eq!(p.width, MIN_WIDTH);
    // Near the bottom: opens upwards, anchored to the field's top edge
    let p = placement((100.0, 760.0, 220.0, 784.0), (1280.0, 800.0));
    assert!(p.up && p.edge == 40.0);
    // Narrow screen: never wider than the viewport and clamped to its edges
    let p = placement((250.0, 100.0, 330.0, 124.0), (360.0, 640.0));
    assert!(
        p.width <= 360.0 - 2.0 * MARGIN
            && p.left + p.width <= 360.0 - MARGIN + 0.001
            && p.left >= MARGIN
    );
    // A wide field gets a popup as wide as itself
    let p = placement((50.0, 100.0, 450.0, 124.0), (1280.0, 800.0));
    assert_eq!(p.width, 400.0);
}
