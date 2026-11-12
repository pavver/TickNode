use super::properties::default_value;
use super::*;

#[test]
fn typed_parsing() {
    assert_eq!(
        parse_text("2,5", &DataType::Number),
        Ok(Some(DataValue::Number(NumberValue::Float(2.5))))
    );
    assert!(parse_text("abc", &DataType::Number).is_err());
    assert_eq!(parse_text("", &DataType::Number), Ok(None));
    assert_eq!(
        parse_text("", &DataType::String),
        Ok(Some(DataValue::String(String::new())))
    );
    assert_eq!(
        parse_text("7", &DataType::Any),
        Ok(Some(DataValue::Number(NumberValue::Int(7))))
    );
    assert_eq!(
        parse_text("\"7\"", &DataType::Any),
        Ok(Some(DataValue::String("7".into())))
    );
    assert_eq!(
        parse_text("on", &DataType::Any),
        Ok(Some(DataValue::String("on".into())))
    );
    assert_eq!(
        value_text(&DataValue::String("7".into()), &DataType::Any),
        "\"7\""
    );
    assert_eq!(
        value_text(&DataValue::String("7".into()), &DataType::String),
        "7"
    );
    assert!(matches!(
        parse_text("07:30", &DataType::Time),
        Ok(Some(DataValue::Time(_)))
    ));
    assert!(parse_text("7:99", &DataType::Time).is_err());
    assert_eq!(default_value("duration").get_type(), DataType::Duration);
}

#[test]
fn tick_text_in_any_fields_is_localized_and_can_be_parsed_back_as_duration() {
    i18n::with_locale(i18n::Locale::Uk, || {
        let value = DataValue::Ticks(10);
        let shown = value_text(&value, &DataType::Any);
        assert_eq!(shown, "10 тактів");
        assert_eq!(parse_text(&shown, &DataType::Duration), Ok(Some(value)));
    });
}
