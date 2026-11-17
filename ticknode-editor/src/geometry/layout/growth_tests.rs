//! Resize height distribution over free text rows.

use super::tests::definition;
use super::*;

#[test]
fn a_filled_input_grows_and_empty_fields_share_equally() {
    let def = definition("example");
    let empty = vec![
        ("first".into(), DataValue::from("")),
        ("second".into(), DataValue::from("")),
    ];
    let natural = NodeLayout::new(&def, &empty);
    let size = Some(NodeSize {
        width: 500.0,
        height: natural.height + 101.0,
    });
    let layout = NodeLayout::sized(&def, &empty, size, &[]);
    assert_eq!(layout.property_height("first"), ROW_H + 33.0);
    assert_eq!(layout.property_height("second"), ROW_H + 33.0);
    assert_eq!(
        layout.pin_height(PinId(1), PinDirection::Input),
        ROW_H + 35.0
    );
    let mut filled = empty.clone();
    filled.push((
        ticknode_shared::input_value_property(PinId(1)),
        DataValue::from("hello"),
    ));
    let layout = NodeLayout::sized(&def, &filled, size, &[]);
    assert_eq!(layout.property_height("first"), ROW_H);
    assert_eq!(
        layout.pin_height(PinId(1), PinDirection::Input),
        ROW_H + 101.0
    );
}

#[test]
fn height_shares_are_whole_pixels_in_proportion_to_weights() {
    assert_eq!(text_weight("", 10.0), 0.0);
    assert_eq!(text_weight("short", 10.0), 1.0);
    assert_eq!(text_weight("twenty one characters", 10.0), 3.0);
    assert_eq!(text_weight("a\n\nb", 10.0), 3.0);
    assert_eq!(
        distribute_height(100.0, &[Some(1.0), None, Some(3.0)]),
        vec![25.0, 0.0, 75.0]
    );
    assert_eq!(
        distribute_height(10.0, &[Some(1.0), Some(1.0), Some(1.0)]),
        vec![3.0, 3.0, 4.0]
    );
    assert_eq!(
        distribute_height(9.0, &[Some(0.0), Some(0.0)]),
        vec![4.0, 5.0]
    );
    assert_eq!(
        distribute_height(9.0, &[None, Some(0.0), Some(2.0)]),
        vec![0.0, 0.0, 9.0]
    );
}

#[test]
fn field_rows_share_a_capped_label_column() {
    let mut def = definition("example");
    def.properties = vec![ticknode_shared::PropertyDefinition::text(
        "long",
        "A long property",
    )];
    let props = [("long".into(), DataValue::from("x"))];
    let layout = NodeLayout::sized(&def, &props, None, &[]);
    // "A long property" is wider than the "Text" input label, so the column is its width.
    let column = width::LABEL_COLUMN_MAX.min(15.0 * width::CHAR_WIDTH);
    assert_eq!(layout.label_width, column.min(0.4 * layout.width));
    // The text input reserves the column before its field.
    assert!(layout.pin_width(PinId(1), PinDirection::Input) >= column + width::FIELD_MIN);
    // A connected input has no field and no column of its own.
    def.properties.clear();
    let wired = NodeLayout::sized(&def, &[], None, &[PinId(1)]);
    assert_eq!(wired.label_width, 0.0);
    // Very long labels cap the column at 40% of the node width.
    def.properties = vec![ticknode_shared::PropertyDefinition::text(
        "l",
        &"Long ".repeat(20),
    )];
    let capped = NodeLayout::sized(&def, &[("l".into(), DataValue::from(""))], None, &[]);
    assert_eq!(
        capped.label_width,
        width::LABEL_COLUMN_MAX.min(0.4 * capped.width)
    );
}
