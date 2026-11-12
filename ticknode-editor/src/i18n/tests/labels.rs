use super::*;
use ticknode_shared::condition::CompareOp;

#[test]
fn ticks_use_locale_plural_rules() {
    with_locale(Locale::En, || {
        assert_eq!(ticks_text(1), "1 tick");
        for count in [0, 2, 10, 21] {
            assert_eq!(ticks_text(count), format!("{count} ticks"));
        }
    });
    with_locale(Locale::Uk, || {
        for (count, expected) in [
            (0, "0 тактів"),
            (1, "1 такт"),
            (2, "2 такти"),
            (5, "5 тактів"),
            (10, "10 тактів"),
            (11, "11 тактів"),
            (21, "21 такт"),
            (22, "22 такти"),
            (112, "112 тактів"),
        ] {
            assert_eq!(ticks_text(count), expected);
            assert_eq!(
                crate::panels::display_value(&DataValue::Ticks(count)),
                expected
            );
        }
    });
}

#[test]
fn locale_codes_badges_and_tags_agree() {
    for (locale, code, badge) in [(Locale::Uk, "uk", "UK"), (Locale::En, "en", "EN")] {
        assert_eq!(locale.code(), code);
        assert_eq!(locale.badge(), badge);
        assert_eq!(Locale::from_tag(&format!(" {badge}-GB ")), Some(locale));
        assert_eq!(locale.toggle().toggle(), locale);
        assert_ne!(locale.toggle(), locale);
    }
    assert_eq!(Locale::from_tag("de"), None);
}

#[test]
fn builtin_type_labels_preserve_unknown_names() {
    with_locale(Locale::En, || {
        for (name, expected) in [
            ("exec", "Execution"),
            ("number", "Number"),
            ("string", "Text"),
            ("bool", "Yes/no"),
            ("time", "Time"),
            ("datetime", "Date and time"),
            ("duration", "Duration"),
            ("color", "Color"),
            ("object", "Object"),
            ("list", "List"),
            ("any", "Anything"),
            ("auto", "Auto"),
            ("plugin.custom", "plugin.custom"),
        ] {
            assert_eq!(type_name_label(name), expected);
        }
    });
}

#[test]
fn operators_and_temporal_labels_are_preserved() {
    with_locale(Locale::En, || {
        for (op, expected) in [
            (CompareOp::Eq, "="),
            (CompareOp::Ne, "≠"),
            (CompareOp::Lt, "<"),
            (CompareOp::Le, "≤"),
            (CompareOp::Gt, ">"),
            (CompareOp::Ge, "≥"),
            (CompareOp::Contains, "contains"),
            (CompareOp::NotContains, "does not contain"),
            (CompareOp::StartsWith, "starts with"),
            (CompareOp::EndsWith, "ends with"),
            (CompareOp::IsEmpty, "is empty"),
            (CompareOp::NotEmpty, "is not empty"),
            (CompareOp::IsTrue, "= true"),
            (CompareOp::IsFalse, "= false"),
            (CompareOp::HasField, "has field"),
            (CompareOp::NoField, "lacks field"),
            (CompareOp::LenEq, "length ="),
            (CompareOp::LenGt, "length >"),
            (CompareOp::LenLt, "length <"),
            (CompareOp::HasValue, "has value"),
            (CompareOp::NoValue, "no value"),
        ] {
            assert_eq!(op_symbol(op), expected);
            assert_eq!(op_label_for(op, &DataType::String), expected);
        }
        for ty in [DataType::Time, DataType::DateTime] {
            for (op, expected) in [
                (CompareOp::Lt, "< earlier"),
                (CompareOp::Le, "≤ at or before"),
                (CompareOp::Gt, "> later"),
                (CompareOp::Ge, "≥ at or after"),
            ] {
                assert_eq!(op_label_for(op, &ty), expected);
            }
        }
        assert_eq!(
            op_label_for(CompareOp::Lt, &DataType::Duration),
            "< shorter"
        );
        assert_eq!(op_label_for(CompareOp::Gt, &DataType::Duration), "> longer");
    });
}

#[test]
fn field_hints_and_parse_errors_follow_the_selected_type() {
    with_locale(Locale::En, || {
        for (ty, hint) in [
            (DataType::Time, "hint-time"),
            (DataType::DateTime, "hint-datetime"),
            (DataType::Duration, "hint-duration"),
            (DataType::Color, "hint-color"),
            (DataType::Any, "hint-default"),
        ] {
            assert_eq!(type_hint(&ty), t!(hint));
        }
        for ty in [
            DataType::Number,
            DataType::Bool,
            DataType::Time,
            DataType::DateTime,
            DataType::Duration,
            DataType::Color,
        ] {
            let error = parse_value_localized(" invalid ", &ty).unwrap_err();
            assert!(error.contains("invalid"), "{error}");
            assert!(
                !error.contains(" invalid "),
                "the input is trimmed: {error}"
            );
        }
        assert!(
            parse_value_localized("invalid", &DataType::Object)
                .unwrap_err()
                .contains("object")
        );
        assert_eq!(
            parse_value_localized("42", &DataType::Number).unwrap(),
            DataValue::Number(NumberValue::Int(42))
        );
        assert_eq!(
            parse_value_localized("hello", &DataType::String).unwrap(),
            DataValue::String("hello".into())
        );
        assert_eq!(join_symbol(condition::Join::And), "and");
        assert_eq!(join_symbol(condition::Join::Or), "or");
        assert_eq!(
            format_match_case(&condition::MatchCase::parse("1:").unwrap()),
            t!("match-always")
        );
        let case = condition::MatchCase::parse("1: a > 5 or b is_true").unwrap();
        assert_eq!(format_match_case(&case), "a > 5 or b = true");
    });
}
