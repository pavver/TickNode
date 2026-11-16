use super::properties::default_value;
use super::*;

#[test]
fn typed_variable_commit_only_emits_a_binding_when_the_field_accepts_variables() {
    let owner = Owner::new();
    owner.with(|| {
        let committed = RwSignal::new(Vec::<Option<String>>::new());
        let callback = Callback::new(move |value| committed.update(|items| items.push(value)));
        assert!(!variable::commit_variable_binding("var.amount", None));
        assert!(!variable::commit_variable_binding("42", Some(callback)));
        assert!(committed.get_untracked().is_empty());
        assert!(variable::commit_variable_binding(
            "  var.amount  ",
            Some(callback)
        ));
        assert_eq!(committed.get_untracked(), [Some("var.amount".into())]);
    });
}

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

#[test]
fn commit_text_decision_for_numbers_strings_and_durations() {
    // Number inputs with allow_variable = true
    assert_eq!(
        decide_input_commit("var.timeout", &DataType::Number, true, true),
        Ok(InputCommit::Variable("var.timeout".into()))
    );
    assert_eq!(
        decide_input_commit("sheet.step", &DataType::Number, true, true),
        Ok(InputCommit::Variable("sheet.step".into()))
    );
    assert_eq!(
        decide_input_commit("global.mode", &DataType::Number, true, true),
        Ok(InputCommit::Variable("global.mode".into()))
    );
    assert_eq!(
        decide_input_commit("42", &DataType::Number, true, true),
        Ok(InputCommit::Value(Some(DataValue::Number(
            NumberValue::Int(42)
        ))))
    );
    assert_eq!(
        decide_input_commit("   ", &DataType::Number, true, true),
        Ok(InputCommit::Value(None))
    );
    // Invalid variable names fail parsing as number as before
    assert!(decide_input_commit("var.", &DataType::Number, true, true).is_err());
    assert!(decide_input_commit("var. 1x", &DataType::Number, true, true).is_err());
    // allow_variable = false
    assert!(decide_input_commit("var.timeout", &DataType::Number, true, false).is_err());

    // String inputs
    assert_eq!(
        decide_input_commit("var.x", &DataType::String, true, true),
        Ok(InputCommit::Variable("var.x".into()))
    );
    assert_eq!(
        decide_input_commit("  var.x  ", &DataType::String, true, true),
        Ok(InputCommit::Variable("var.x".into()))
    );
    // Quoted literal for String: outer pair of quotes stripped
    assert_eq!(
        decide_input_commit("\"var.x\"", &DataType::String, true, true),
        Ok(InputCommit::Value(Some(DataValue::String("var.x".into()))))
    );
    assert_eq!(
        decide_input_commit("\"hello\"", &DataType::String, true, true),
        Ok(InputCommit::Value(Some(DataValue::String("hello".into()))))
    );
    assert_eq!(
        decide_input_commit("hello", &DataType::String, true, true),
        Ok(InputCommit::Value(Some(DataValue::String("hello".into()))))
    );
    // Invalid variable references stay literal string
    assert_eq!(
        decide_input_commit("var.", &DataType::String, true, true),
        Ok(InputCommit::Value(Some(DataValue::String("var.".into()))))
    );
    assert_eq!(
        decide_input_commit("var. 1x", &DataType::String, true, true),
        Ok(InputCommit::Value(Some(DataValue::String(
            "var. 1x".into()
        ))))
    );
    assert_eq!(
        decide_input_commit("   ", &DataType::String, true, true),
        Ok(InputCommit::Value(None))
    );
    // allow_variable = false keeps raw text without stripping quotes
    assert_eq!(
        decide_input_commit("var.x", &DataType::String, true, false),
        Ok(InputCommit::Value(Some(DataValue::String("var.x".into()))))
    );
    assert_eq!(
        decide_input_commit("\"var.x\"", &DataType::String, true, false),
        Ok(InputCommit::Value(Some(DataValue::String(
            "\"var.x\"".into()
        ))))
    );

    // Duration inputs
    assert_eq!(
        decide_input_commit("var.delay", &DataType::Duration, true, true),
        Ok(InputCommit::Variable("var.delay".into()))
    );
    assert_eq!(
        decide_input_commit("10s", &DataType::Duration, true, true),
        Ok(InputCommit::Value(Some(DataValue::Duration(
            chrono::TimeDelta::seconds(10)
        ))))
    );
    assert!(decide_input_commit("var.", &DataType::Duration, true, true).is_err());
}

#[test]
fn variable_resolution_and_sheet_shadowing() {
    use std::collections::BTreeMap;

    let s1 = SheetId::generate();
    let s2 = SheetId::generate();

    let mut map = BTreeMap::new();
    let sheet_var = Variable {
        key: VariableKey::sheet(s1, "temp"),
        data_type: DataType::Number,
        value: DataValue::from(25i64),
    };
    let global_var = Variable {
        key: VariableKey::global("temp"),
        data_type: DataType::Number,
        value: DataValue::from(10i64),
    };
    let persistent_var = Variable {
        key: VariableKey::persistent("db"),
        data_type: DataType::String,
        value: DataValue::from("sqlite"),
    };
    let s2_var = Variable {
        key: VariableKey::sheet(s2, "target"),
        data_type: DataType::Number,
        value: DataValue::from(100i64),
    };

    map.insert(sheet_var.key.clone(), sheet_var.clone());
    map.insert(global_var.key.clone(), global_var.clone());
    map.insert(persistent_var.key.clone(), persistent_var.clone());
    map.insert(s2_var.key.clone(), s2_var.clone());

    // Auto lookup: var.temp on s1 resolves to sheet_var (shadows global!)
    let r_auto_temp = VariableRef::auto("temp");
    assert_eq!(
        resolve_variable(&r_auto_temp, Some(s1), &map),
        Some(&sheet_var)
    );
    // Auto lookup: var.temp on s2 falls back to global_var
    assert_eq!(
        resolve_variable(&r_auto_temp, Some(s2), &map),
        Some(&global_var)
    );
    // Auto lookup: var.temp with no open sheet falls back to global_var
    assert_eq!(
        resolve_variable(&r_auto_temp, None, &map),
        Some(&global_var)
    );

    // Auto lookup: persistent variable
    let r_auto_db = VariableRef::auto("db");
    assert_eq!(
        resolve_variable(&r_auto_db, Some(s1), &map),
        Some(&persistent_var)
    );

    // Sheet lookup: only matches open sheet
    let r_sheet_temp = VariableRef::sheet("temp");
    assert_eq!(
        resolve_variable(&r_sheet_temp, Some(s1), &map),
        Some(&sheet_var)
    );
    assert_eq!(resolve_variable(&r_sheet_temp, Some(s2), &map), None);
    assert_eq!(resolve_variable(&r_sheet_temp, None, &map), None);

    // Global lookup: ignores sheet variable on s1
    let r_global_temp = VariableRef::global("temp");
    assert_eq!(
        resolve_variable(&r_global_temp, Some(s1), &map),
        Some(&global_var)
    );
    let r_global_db = VariableRef::global("db");
    assert_eq!(
        resolve_variable(&r_global_db, Some(s1), &map),
        Some(&persistent_var)
    );
    let r_global_target = VariableRef::global("target");
    assert_eq!(resolve_variable(&r_global_target, Some(s1), &map), None);

    // Missing variable
    let r_missing = VariableRef::auto("missing");
    assert_eq!(resolve_variable(&r_missing, Some(s1), &map), None);
}

#[test]
fn variable_tooltip_presentation() {
    let types = TypeRegistry::new();
    let var = Variable {
        key: VariableKey::global("temp"),
        data_type: DataType::Number,
        value: DataValue::from(25i64),
    };
    let vref = VariableRef::auto("temp");
    let missing_ref = VariableRef::auto("missing");

    i18n::with_locale(i18n::Locale::En, || {
        assert_eq!(
            variable_tooltip(&vref, Some((&var.value, &var.data_type)), &types),
            "Variable «temp» (Number): 25"
        );
        assert_eq!(
            variable_tooltip(&missing_ref, None, &types),
            "Variable «missing» not found"
        );
    });

    i18n::with_locale(i18n::Locale::Uk, || {
        assert_eq!(
            variable_tooltip(&vref, Some((&var.value, &var.data_type)), &types),
            "Змінна «temp» (Число): 25"
        );
        assert_eq!(
            variable_tooltip(&missing_ref, None, &types),
            "Змінну «missing» не знайдено"
        );
    });
}

#[test]
fn required_missing_respects_variable_binding() {
    use crate::state::is_required_missing;

    let req_pin = PinDefinition {
        id: PinId(1),
        name: "Input".into(),
        direction: PinDirection::Input,
        pin_type: PinType::Data(DataType::Number),
        required: true,
    };
    let opt_pin = PinDefinition {
        id: PinId(2),
        name: "Optional".into(),
        direction: PinDirection::Input,
        pin_type: PinType::Data(DataType::Number),
        required: false,
    };
    let exec_pin = PinDefinition {
        id: PinId(3),
        name: "In".into(),
        direction: PinDirection::Input,
        pin_type: PinType::Exec,
        required: true,
    };

    // Unconnected required pin with no inline value or binding -> missing
    assert!(is_required_missing(&req_pin, false, &[]));
    // Connected required pin -> not missing
    assert!(!is_required_missing(&req_pin, true, &[]));
    // Unconnected required pin with inline value in:1 -> not missing
    let in_props = [(input_value_property(PinId(1)), DataValue::from(42i64))];
    assert!(!is_required_missing(&req_pin, false, &in_props));
    // Unconnected required pin with variable binding var:1 -> not missing!
    let var_props = [(
        input_variable_property(PinId(1)),
        DataValue::String("var.x".into()),
    )];
    assert!(!is_required_missing(&req_pin, false, &var_props));

    // Optional and exec pins are never missing
    assert!(!is_required_missing(&opt_pin, false, &[]));
    assert!(!is_required_missing(&exec_pin, false, &[]));
}
