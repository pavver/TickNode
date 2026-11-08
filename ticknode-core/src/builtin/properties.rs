use ticknode_shared::{DataType, DataValue, parse_data_type};

/// Collection type properties accept strings, default invalid names to Number, and ignore other values.
pub(super) fn set_type_property(target: &mut DataType, property: &str, name: &str, value: DataValue) {
    if name == property && let DataValue::String(text) = value {
        *target = parse_data_type(&text).unwrap_or(DataType::Number);
    }
}
