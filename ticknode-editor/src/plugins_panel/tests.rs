use super::*;

#[test]
fn form_values_map_to_updates() {
    let text = SettingDefinition::text("greeting", "Greeting");
    let secret = SettingDefinition::secret("token", "Token");
    let number = SettingDefinition::number("port", "Port");
    let flag = SettingDefinition::bool("tls", "TLS");
    assert_eq!(update_of("p", &text, "hi", false), Ok(SettingUpdate::Set(DataValue::String("hi".into()))));
    assert_eq!(update_of("p", &text, "", false), Ok(SettingUpdate::Clear));
    // Secret: empty field means "keep unchanged", explicit clear means Clear
    assert_eq!(update_of("p", &secret, "", false), Ok(SettingUpdate::Keep));
    assert_eq!(update_of("p", &secret, "", true), Ok(SettingUpdate::Clear));
    assert_eq!(update_of("p", &secret, "abc", false), Ok(SettingUpdate::Set(DataValue::String("abc".into()))));
    assert_eq!(update_of("p", &number, "8123", false), Ok(SettingUpdate::Set(DataValue::Number(NumberValue::Int(8123)))));
    assert!(update_of("p", &number, "abc", false).is_err());
    assert_eq!(update_of("p", &number, " ", false), Ok(SettingUpdate::Clear));
    assert_eq!(update_of("p", &flag, "true", false), Ok(SettingUpdate::Set(DataValue::Bool(true))));
    assert_eq!(update_of("p", &flag, "", false), Ok(SettingUpdate::Set(DataValue::Bool(false))));
}

#[test]
fn secrets_never_prefill_the_form() {
    assert_eq!(draft_of(&SettingValueView::Secret { is_set: true }), "");
    assert_eq!(draft_of(&SettingValueView::Value(DataValue::Bool(true))), "true");
    assert_eq!(draft_of(&SettingValueView::Value(DataValue::String("x".into()))), "x");
}
