use super::*;

#[test]
fn any_input_displays_source_type_when_connected_and_any_when_disconnected() {
    assert_eq!(display_type(&DataType::Any, None), DataType::Any);
    assert_eq!(display_type(&DataType::Any, Some(DataType::Number)), DataType::Number);
    assert_eq!(display_type(&DataType::Any, Some(DataType::String)), DataType::String);
    assert_eq!(display_type(&DataType::Any, Some(DataType::Bool)), DataType::Bool);
    assert_eq!(display_type(&DataType::Number, Some(DataType::String)), DataType::Number);
    assert_eq!(display_type(&DataType::String, None), DataType::String);
}

#[test]
fn tooltips_keep_required_and_technical_type_information() {
    i18n::with_locale(i18n::Locale::En, || {
        assert_eq!(pin_tooltip("Go", "Execution", &PinType::Exec, false), "Go: Execution");
        assert_eq!(pin_tooltip("Value", "Number", &PinType::Data(DataType::Number), true),
            format!("Value: Number{}\n{}", t!("pin-required-suffix"), t!("pin-technical-type", name = "number")));
    });
}

#[test]
fn only_editable_data_inputs_get_inline_values() {
    let types = TypeRegistry::new();
    assert_eq!(inline_type(&types, PinDirection::Input, &PinType::Data(DataType::Number)), Some(DataType::Number));
    assert_eq!(inline_type(&types, PinDirection::Output, &PinType::Data(DataType::Number)), None);
    assert_eq!(inline_type(&types, PinDirection::Input, &PinType::Exec), None);
    assert_eq!(inline_type(&types, PinDirection::Input, &PinType::Data(DataType::Object)), None);
}

#[test]
fn pin_classes_keep_all_states_in_one_attribute() {
    assert_eq!(pin_class(true, false, false, false), "pin in");
    assert_eq!(pin_class(false, false, false, false), "pin out");
    assert_eq!(pin_class(true, true, true, true), "pin in split-field missing connectable");
    assert_eq!(pin_class(false, true, false, true), "pin out split-field connectable");
}
