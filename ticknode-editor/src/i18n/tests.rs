use super::*;
use ticknode_shared::condition::CompareOp;

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
        for (name, expected) in [("exec", "Execution"), ("number", "Number"), ("string", "Text"),
            ("bool", "Yes/no"), ("time", "Time"), ("datetime", "Date and time"),
            ("duration", "Duration"), ("color", "Color"), ("object", "Object"),
            ("list", "List"), ("any", "Anything"), ("auto", "Auto"), ("plugin.custom", "plugin.custom")] {
            assert_eq!(type_name_label(name), expected);
        }
    });
}

#[test]
fn operators_and_temporal_labels_are_preserved() {
    with_locale(Locale::En, || {
        for (op, expected) in [(CompareOp::Eq, "="), (CompareOp::Ne, "≠"), (CompareOp::Lt, "<"),
            (CompareOp::Le, "≤"), (CompareOp::Gt, ">"), (CompareOp::Ge, "≥"),
            (CompareOp::Contains, "contains"), (CompareOp::NotContains, "does not contain"),
            (CompareOp::StartsWith, "starts with"), (CompareOp::EndsWith, "ends with"),
            (CompareOp::IsEmpty, "is empty"), (CompareOp::NotEmpty, "is not empty"),
            (CompareOp::IsTrue, "= true"), (CompareOp::IsFalse, "= false"),
            (CompareOp::HasField, "has field"), (CompareOp::NoField, "lacks field"),
            (CompareOp::LenEq, "length ="), (CompareOp::LenGt, "length >"), (CompareOp::LenLt, "length <"),
            (CompareOp::HasValue, "has value"), (CompareOp::NoValue, "no value")] {
            assert_eq!(op_symbol(op), expected);
            assert_eq!(op_label_for(op, &DataType::String), expected);
        }
        for ty in [DataType::Time, DataType::DateTime] {
            for (op, expected) in [(CompareOp::Lt, "< earlier"), (CompareOp::Le, "≤ at or before"),
                (CompareOp::Gt, "> later"), (CompareOp::Ge, "≥ at or after")] {
                assert_eq!(op_label_for(op, &ty), expected);
            }
        }
        assert_eq!(op_label_for(CompareOp::Lt, &DataType::Duration), "< shorter");
        assert_eq!(op_label_for(CompareOp::Gt, &DataType::Duration), "> longer");
    });
}

#[test]
fn field_hints_and_parse_errors_follow_the_selected_type() {
    with_locale(Locale::En, || {
        for (ty, hint) in [(DataType::Time, "hint-time"), (DataType::DateTime, "hint-datetime"),
            (DataType::Duration, "hint-duration"), (DataType::Color, "hint-color"), (DataType::Any, "hint-default")] {
            assert_eq!(type_hint(&ty), t!(hint));
        }
        for ty in [DataType::Number, DataType::Bool, DataType::Time, DataType::DateTime, DataType::Duration, DataType::Color] {
            let error = parse_value_localized(" invalid ", &ty).unwrap_err();
            assert!(error.contains("invalid"), "{error}");
            assert!(!error.contains(" invalid "), "the input is trimmed: {error}");
        }
        assert!(parse_value_localized("invalid", &DataType::Object).unwrap_err().contains("object"));
        assert_eq!(parse_value_localized("42", &DataType::Number).unwrap(), DataValue::Number(NumberValue::Int(42)));
        assert_eq!(parse_value_localized("hello", &DataType::String).unwrap(), DataValue::String("hello".into()));
        assert_eq!(join_symbol(condition::Join::And), "and");
        assert_eq!(join_symbol(condition::Join::Or), "or");
        assert_eq!(format_match_case(&condition::MatchCase::parse("1:").unwrap()), t!("match-always"));
        let case = condition::MatchCase::parse("1: a > 5 or b is_true").unwrap();
        assert_eq!(format_match_case(&case), "a > 5 or b = true");
    });
}
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    fn extract_ftl_keys(source: &str) -> BTreeSet<String> {
        let ast = fluent_syntax::parser::parse(source).expect("FTL must parse without syntax errors");
        let mut keys = BTreeSet::new();
        for entry in ast.body {
            if let fluent_syntax::ast::Entry::Message(msg) = entry {
                assert!(
                    keys.insert(msg.id.name.to_string()),
                    "duplicate key in FTL: {}",
                    msg.id.name
                );
            }
        }
        keys
    }

    #[test]
    fn ftl_files_parse_and_have_identical_keys() {
        let uk_ui = extract_ftl_keys(UK_UI_FTL);
        let en_ui = extract_ftl_keys(EN_UI_FTL);
        let only_uk_ui: Vec<_> = uk_ui.difference(&en_ui).collect();
        let only_en_ui: Vec<_> = en_ui.difference(&uk_ui).collect();
        assert!(
            only_uk_ui.is_empty() && only_en_ui.is_empty(),
            "ui.ftl keys differ: only in uk={only_uk_ui:?}, only in en={only_en_ui:?}"
        );

        // Verify bundle initialization for both locales
        BUNDLES.with(|b| {
            let b = b.borrow();
            for key in &uk_ui {
                assert!(b.uk.has_message(key), "no key {key} in the uk bundle");
                assert!(b.en.has_message(key), "no key {key} in the en bundle");
            }
        });
    }

    /// Finds all translation macro calls in a source file (ignoring comment lines).
    fn scan_t_calls(src: &str) -> Vec<String> {
        let cleaned: String = src
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut keys = Vec::new();
        let bytes = cleaned.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b't' && bytes[i + 1] == b'!' && bytes[i + 2] == b'(' {
                let prev_ok =
                    i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                if prev_ok {
                    let mut j = i + 3;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'"' {
                        let start = j + 1;
                        let mut end = start;
                        while end < bytes.len() && bytes[end] != b'"' {
                            if bytes[end] == b'\\' {
                                end += 2;
                            } else {
                                end += 1;
                            }
                        }
                        if end < bytes.len()
                            && let Ok(key) = std::str::from_utf8(&bytes[start..end])
                            && !key.is_empty()
                        {
                            keys.push(key.to_string());
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            i += 1;
        }
        keys
    }

    #[test]
    fn every_t_key_used_in_code_exists_in_bundles() {
        let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut files = Vec::new();
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).expect("a source directory exists") {
                let path = entry.expect("a directory entry").path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        let mut checked = 0usize;
        for path in files {
            let content = fs::read_to_string(&path).expect("reading a .rs file");
            for key in scan_t_calls(&content) {
                BUNDLES.with(|b| {
                    let b = b.borrow();
                    assert!(b.uk.has_message(&key), "key '{key}' from {} is missing in locales/uk/ui.ftl", path.display());
                    assert!(b.en.has_message(&key), "key '{key}' from {} is missing in locales/en/ui.ftl", path.display());
                });
                checked += 1;
            }
        }
        assert!(checked > 0, "the scanner must find translation macro calls in the source");
    }

    fn def(type_name: &str) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: type_name.into(),
            title: "Weather Forecast".into(),
            category: "Weather".into(),
            description: "Fetches the forecast".into(),
            help: "Detailed plugin help".into(),
            pins: Vec::new(),
        }
    }

    const PLUGIN_UK: &str = "\
node-weather-forecast-title = Прогноз погоди
node-weather-forecast-description = Отримує прогноз
node-weather-forecast-pin-city = Місто
node-weather-forecast-pin-delay-ms = Затримка
type-weather-state-title = Стан
type-weather-state-field-temp = Температура
type-weather-mode-value-auto = Авто
setting-weather-plugin-url-title = Адреса
weather-plugin-error-no-token = Токен не задано ({ $name })
";

    #[test]
    fn without_translations_the_definition_texts_are_used() {
        let plugin_def = def("weather/forecast");
        with_locale(Locale::En, || {
            assert_eq!(node_title(&plugin_def), "Weather Forecast");
            assert_eq!(node_description(&plugin_def), "Fetches the forecast");
            assert_eq!(node_help(&plugin_def), "Detailed plugin help");
            assert_eq!(node_pin_label("weather/forecast", "City"), "City");
        });
    }

    #[test]
    fn runtime_resources_translate_nodes_types_and_errors() {
        set_runtime_translations(&[("uk".to_string(), PLUGIN_UK.to_string())]);
        let plugin_def = def("weather/forecast");
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&plugin_def), "Прогноз погоди");
            assert_eq!(node_description(&plugin_def), "Отримує прогноз");
            // No help key: the definition's text stays
            assert_eq!(node_help(&plugin_def), "Detailed plugin help");
            // The pin label is looked up by the name as written, then by its slug
            assert_eq!(node_pin_label("weather/forecast", "city"), "Місто");
            assert_eq!(node_pin_label("weather/forecast", "Delay ms"), "Затримка");
            assert_eq!(node_pin_label("weather/forecast", "Other"), "Other");
            let setting = SettingDefinition::text("url", "URL");
            assert_eq!(setting_title("weather_plugin", &setting), "Адреса");
            assert_eq!(setting_description("weather_plugin", &setting), "");
            let value = EnumValue { value: "auto".into(), title: "Auto".into() };
            assert_eq!(enum_value_title("weather.mode", &value), "Авто");
            assert_eq!(enum_value_title("weather.mode", &EnumValue { value: "manual".into(), title: "Manual".into() }), "Manual");
            let msg = UiMessage::new("weather-plugin-error-no-token", "Token is not set").arg("name", "x");
            assert_eq!(ui_text(&msg), "Токен не задано (x)");
            // An unknown key or no key: the English fallback
            assert_eq!(ui_text(&UiMessage::new("weather-plugin-nope", "Fallback")), "Fallback");
            assert_eq!(ui_text(&UiMessage::plain("Plain")), "Plain");
        });
        // The plugin has no `en` resource: the English locale uses the definition
        with_locale(Locale::En, || {
            assert_eq!(node_title(&plugin_def), "Weather Forecast");
            assert_eq!(ui_text(&UiMessage::new("weather-plugin-error-no-token", "Token is not set")), "Token is not set");
        });
    }

    #[test]
    fn lookup_falls_back_from_the_locale_to_en() {
        set_runtime_translations(&[
            ("en".to_string(), "node-weather-forecast-title = Forecast\nnode-weather-forecast-help = English help".to_string()),
            ("uk".to_string(), "node-weather-forecast-title = Прогноз".to_string()),
        ]);
        let plugin_def = def("weather/forecast");
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&plugin_def), "Прогноз");
            // Missing in uk: taken from en before the definition's text
            assert_eq!(node_help(&plugin_def), "English help");
            assert_eq!(node_description(&plugin_def), "Fetches the forecast");
        });
        with_locale(Locale::En, || assert_eq!(node_title(&plugin_def), "Forecast"));
    }

    #[test]
    fn property_texts_come_from_plugin_resources_with_definition_fallbacks() {
        set_runtime_translations(&[
            ("uk".to_string(), "node-weather-forecast-prop-entity_id-title = Сутність\nnode-weather-forecast-prop-mode-option-latest = Лише остання\nnode-weather-forecast-prop-mode-option-every-change = Кожна".to_string()),
        ]);
        let entity = PropertyDefinition::dynamic("entity_id", "Entity", "entities").described("Pick one");
        let mode = PropertyDefinition::select("mode", "Mode", &[("every-change", "Every"), ("latest", "Latest only")]);
        with_locale(Locale::Uk, || {
            assert_eq!(property_title("weather/forecast", &entity), "Сутність");
            // No translation: the definition's English text
            assert_eq!(property_title("weather/forecast", &mode), "Mode");
            assert_eq!(property_description("weather/forecast", &entity), "Pick one");
            assert_eq!(property_option_label("weather/forecast", "mode", "latest", "Latest only"), "Лише остання");
            // Option values are slugified in keys
            assert_eq!(property_option_label("weather/forecast", "mode", "every-change", "Every"), "Кожна");
            assert_eq!(property_option_label("weather/forecast", "mode", "other", "Other"), "Other");
        });
        with_locale(Locale::En, || {
            assert_eq!(property_title("weather/forecast", &entity), "Entity");
            assert_eq!(property_option_label("weather/forecast", "mode", "latest", "Latest only"), "Latest only");
        });
    }

    #[test]
    fn the_editor_ui_texts_win_over_runtime_resources() {
        set_runtime_translations(&[("en".to_string(), "toolbar-plugins = HIJACKED\ncategory-extra = Extra".to_string())]);
        with_locale(Locale::En, || {
            assert_eq!(t!("toolbar-plugins"), "Plugins");
            assert_eq!(translate("category-extra", None), "Extra");
        });
    }

    #[test]
    fn broken_runtime_resources_do_not_break_the_bundles() {
        set_runtime_translations(&[("en".to_string(), "good = ok\nthis is = = broken {".to_string())]);
        with_locale(Locale::En, || {
            assert_eq!(translate("good", None), "ok");
            assert_eq!(t!("toolbar-plugins"), "Plugins");
        });
    }

    #[test]
    fn make_and_break_nodes_use_the_translated_type() {
        let mut registry = TypeRegistry::new();
        registry.insert_all(&[TypeDefinition::new_struct(
            "weather.state",
            "State",
            vec![FieldDefinition::new("temp", "Temp", DataType::Number)],
        )]).unwrap();
        set_known_types(&registry);
        set_runtime_translations(&[("uk".to_string(), PLUGIN_UK.to_string())]);
        let make = NodeDefinition { type_name: "type/make/weather.state".into(), title: "Make State".into(), ..def("x") };
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&make), "Створити Стан");
            assert_eq!(node_description(&make), "Збирає значення «Стан» з полів");
            assert_eq!(node_pin_label("type/make/weather.state", "State"), "Стан");
            assert_eq!(node_pin_label("type/make/weather.state", "Temp"), "Температура");
            assert_eq!(node_pin_label("type/break/weather.state", "Temp"), "Температура");
        });
        // No translated type: the server's text is kept
        with_locale(Locale::En, || assert_eq!(node_title(&make), "Make State"));
    }

    #[test]
    fn type_display_titles_are_localized_and_recursive() {
        let types = TypeRegistry::from_definitions([TypeDefinition::new_struct("test.point", "Point", vec![])]);
        with_locale(Locale::Uk, || {
            for (ty, title) in [
                (DataType::Number, "Число"), (DataType::String, "Текст"), (DataType::Bool, "Так/ні"),
                (DataType::Time, "Час"), (DataType::DateTime, "Дата й час"), (DataType::Duration, "Тривалість"),
                (DataType::Color, "Колір"), (DataType::Object, "Об’єкт"), (DataType::Any, "Будь-що"),
            ] { assert_eq!(data_type_title(&types, &ty), title); }
            assert_eq!(data_type_title(&types, &DataType::List(Box::new(DataType::Number))), "Список: Число");
            assert_eq!(data_type_title(&types, &DataType::Named("test.point".into())), "Point");
            assert_eq!(data_type_title(&types, &DataType::Named("missing.type".into())), "Недоступний тип");
        });
        with_locale(Locale::En, || {
            assert_eq!(data_type_title(&types, &DataType::List(Box::new(DataType::String))), "List: Text");
        });
        set_runtime_translations(&[("uk".into(), "type-test-point-title = Точка".into())]);
        with_locale(Locale::Uk, || assert_eq!(data_type_title(&types, &DataType::Named("test.point".into())), "Точка"));
        set_runtime_translations(&[]);
    }

    #[test]
    fn plurals_and_arguments_work_in_both_locales() {
        with_locale(Locale::Uk, || {
            assert_eq!(t!("time-seconds-ago", count = 1), "1 секунду тому");
            assert_eq!(t!("time-seconds-ago", count = 2), "2 секунди тому");
            assert_eq!(t!("time-seconds-ago", count = 5), "5 секунд тому");
            assert_eq!(t!("time-seconds-ago", count = 21), "21 секунду тому");
            assert_eq!(t!("group-selected", count = 1), "Виділено 1 ноду");
            assert_eq!(t!("group-selected", count = 3), "Виділено 3 ноди");
            assert_eq!(t!("group-selected", count = 12), "Виділено 12 нод");
        });
        with_locale(Locale::En, || {
            assert_eq!(t!("time-seconds-ago", count = 1), "1 second ago");
            assert_eq!(t!("time-seconds-ago", count = 2), "2 seconds ago");
            assert_eq!(t!("time-seconds-ago", count = 5), "5 seconds ago");
            assert_eq!(t!("group-selected", count = 1), "1 node selected");
            assert_eq!(t!("group-selected", count = 3), "3 nodes selected");
            assert_eq!(t!("status-online", count = 1), "connected · 1 node");
            assert_eq!(t!("status-online", count = 4), "connected · 4 nodes");
        });
    }
