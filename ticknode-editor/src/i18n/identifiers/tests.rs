use super::*;
use crate::i18n::{self, Locale, with_locale};

fn setup() -> DataType {
    let mut chat = TypeDefinition::identifier("opaque.chat", "Chat", "").labels("ids");
    chat.plugin = Some("messaging".into());
    i18n::set_known_types(&TypeRegistry::from_definitions([chat]));
    set_identifier_options(
        &[
            (
                ("messaging".into(), "ids".into()),
                Arc::new(vec![PropertyOption {
                    value: "-1001".into(),
                    label: "Alice".into(),
                    detail: String::new(),
                    icon: None,
                }]),
            ),
            (
                ("other".into(), "ids".into()),
                Arc::new(vec![PropertyOption {
                    value: "-1001".into(),
                    label: "Wrong plugin".into(),
                    detail: String::new(),
                    icon: None,
                }]),
            ),
        ]
        .into(),
    );
    DataType::Named("opaque.chat".into())
}

#[test]
fn live_identifier_labels_reach_wires_traces_match_and_variable_search() {
    let t = setup();
    for locale in [Locale::En, Locale::Uk] {
        with_locale(locale, || {
            assert_eq!(
                identifier_text(&(-1001i64).into(), &t).as_deref(),
                Some("Alice (-1001)")
            );
            assert_eq!(i18n::value_as_text(&(-1001i64).into(), &t), "Alice (-1001)");
            assert_eq!(
                crate::panels::display_value_in(&(-1001i64).into(), &t),
                "Alice (-1001)"
            );
            assert_eq!(
                i18n::value_as_text(&i64::MAX.into(), &t),
                i64::MAX.to_string()
            );
            let case = condition::MatchCase::parse("1: chat = -1001").unwrap();
            let inputs = [condition::MatchInput::parse("1:chat:opaque.chat").unwrap()];
            assert!(i18n::format_match_case_in(&case, &inputs).contains("Alice (-1001)"));
            assert_eq!(
                i18n::parse_value_localized(&i64::MAX.to_string(), &t).unwrap(),
                i64::MAX.into()
            );
            for invalid in ["1.5", "9223372036854775808", "Alice"] {
                let error = i18n::parse_value_localized(invalid, &t).unwrap_err();
                assert!(!error.contains("parse-error-identifier"));
            }
        });
    }
    let variable = Variable {
        key: VariableKey::global("destination"),
        data_type: t.clone(),
        value: (-1001i64).into(),
    };
    let types =
        TypeRegistry::from_definitions([TypeDefinition::identifier("opaque.chat", "Chat", "")]);
    let groups = crate::variables_panel::logic::group_and_filter_variables(
        &[variable],
        None,
        "Alice -1001",
        &types,
    );
    assert_eq!(groups.global.len(), 1);
    set_identifier_options(&Default::default());
    assert_eq!(i18n::value_as_text(&(-1001i64).into(), &t), "-1001");
    i18n::set_known_types(&TypeRegistry::new());
}

#[test]
fn identifier_picker_constants_keep_the_integer_and_offer_the_owning_source() {
    let t = setup();
    assert_eq!(
        identifier_source(&t),
        Some(("messaging".into(), "ids".into()))
    );
    let types =
        TypeRegistry::from_definitions([
            TypeDefinition::identifier("opaque.chat", "Chat", "").labels("ids")
        ]);
    assert_eq!(
        text::parse_value_in(&types, "-1001", &t).unwrap(),
        (-1001i64).into()
    );
    assert!(input_value_fits(&types, &t, &(-1001i64).into()));
    assert!(!input_value_fits(&types, &t, &"-1001".into()));
    set_identifier_options(&Default::default());
    i18n::set_known_types(&TypeRegistry::new());
}
