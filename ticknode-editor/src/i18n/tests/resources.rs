use super::*;

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
        let value = EnumValue {
            value: "auto".into(),
            title: "Auto".into(),
        };
        assert_eq!(enum_value_title("weather.mode", &value), "Авто");
        assert_eq!(
            enum_value_title(
                "weather.mode",
                &EnumValue {
                    value: "manual".into(),
                    title: "Manual".into()
                }
            ),
            "Manual"
        );
        let msg =
            UiMessage::new("weather-plugin-error-no-token", "Token is not set").arg("name", "x");
        assert_eq!(ui_text(&msg), "Токен не задано (x)");
        // An unknown key or no key: the English fallback
        assert_eq!(
            ui_text(&UiMessage::new("weather-plugin-nope", "Fallback")),
            "Fallback"
        );
        assert_eq!(ui_text(&UiMessage::plain("Plain")), "Plain");
    });
    // The plugin has no `en` resource: the English locale uses the definition
    with_locale(Locale::En, || {
        assert_eq!(node_title(&plugin_def), "Weather Forecast");
        assert_eq!(
            ui_text(&UiMessage::new(
                "weather-plugin-error-no-token",
                "Token is not set"
            )),
            "Token is not set"
        );
    });
}

#[test]
fn lookup_falls_back_from_the_locale_to_en() {
    set_runtime_translations(&[
        (
            "en".to_string(),
            "node-weather-forecast-title = Forecast\nnode-weather-forecast-help = English help"
                .to_string(),
        ),
        (
            "uk".to_string(),
            "node-weather-forecast-title = Прогноз".to_string(),
        ),
    ]);
    let plugin_def = def("weather/forecast");
    with_locale(Locale::Uk, || {
        assert_eq!(node_title(&plugin_def), "Прогноз");
        // Missing in uk: taken from en before the definition's text
        assert_eq!(node_help(&plugin_def), "English help");
        assert_eq!(node_description(&plugin_def), "Fetches the forecast");
    });
    with_locale(Locale::En, || {
        assert_eq!(node_title(&plugin_def), "Forecast")
    });
}

#[test]
fn property_texts_come_from_plugin_resources_with_definition_fallbacks() {
    set_runtime_translations(&[
            ("uk".to_string(), "node-weather-forecast-prop-entity_id-title = Сутність\nnode-weather-forecast-prop-mode-option-latest = Лише остання\nnode-weather-forecast-prop-mode-option-every-change = Кожна".to_string()),
        ]);
    let entity =
        PropertyDefinition::dynamic("entity_id", "Entity", "entities").described("Pick one");
    let mode = PropertyDefinition::select(
        "mode",
        "Mode",
        &[("every-change", "Every"), ("latest", "Latest only")],
    );
    with_locale(Locale::Uk, || {
        assert_eq!(property_title("weather/forecast", &entity), "Сутність");
        // No translation: the definition's English text
        assert_eq!(property_title("weather/forecast", &mode), "Mode");
        assert_eq!(
            property_description("weather/forecast", &entity),
            "Pick one"
        );
        assert_eq!(
            property_option_label("weather/forecast", "mode", "latest", "Latest only"),
            "Лише остання"
        );
        // Option values are slugified in keys
        assert_eq!(
            property_option_label("weather/forecast", "mode", "every-change", "Every"),
            "Кожна"
        );
        assert_eq!(
            property_option_label("weather/forecast", "mode", "other", "Other"),
            "Other"
        );
    });
    with_locale(Locale::En, || {
        assert_eq!(property_title("weather/forecast", &entity), "Entity");
        assert_eq!(
            property_option_label("weather/forecast", "mode", "latest", "Latest only"),
            "Latest only"
        );
    });
}

#[test]
fn the_editor_ui_texts_win_over_runtime_resources() {
    set_runtime_translations(&[(
        "en".to_string(),
        "toolbar-plugins = HIJACKED\ncategory-extra = Extra".to_string(),
    )]);
    with_locale(Locale::En, || {
        assert_eq!(t!("toolbar-plugins"), "Plugins");
        assert_eq!(translate("category-extra", None), "Extra");
    });
}

#[test]
fn broken_runtime_resources_do_not_break_the_bundles() {
    set_runtime_translations(&[(
        "en".to_string(),
        "good = ok\nthis is = = broken {".to_string(),
    )]);
    with_locale(Locale::En, || {
        assert_eq!(translate("good", None), "ok");
        assert_eq!(t!("toolbar-plugins"), "Plugins");
    });
}

#[test]
fn make_and_break_nodes_use_the_translated_type() {
    let mut registry = TypeRegistry::new();
    registry
        .insert_all(&[TypeDefinition::new_struct(
            "weather.state",
            "State",
            vec![FieldDefinition::new("temp", "Temp", DataType::Number)],
        )])
        .unwrap();
    set_known_types(&registry);
    set_runtime_translations(&[("uk".to_string(), PLUGIN_UK.to_string())]);
    let make = NodeDefinition {
        type_name: "type/make/weather.state".into(),
        title: "Make State".into(),
        ..def("x")
    };
    with_locale(Locale::Uk, || {
        assert_eq!(node_title(&make), "Створити Стан");
        assert_eq!(node_description(&make), "Збирає значення «Стан» з полів");
        assert_eq!(node_pin_label("type/make/weather.state", "State"), "Стан");
        assert_eq!(
            node_pin_label("type/make/weather.state", "Temp"),
            "Температура"
        );
        assert_eq!(
            node_pin_label("type/break/weather.state", "Temp"),
            "Температура"
        );
    });
    // No translated type: the server's text is kept
    with_locale(Locale::En, || assert_eq!(node_title(&make), "Make State"));
}

#[test]
fn type_display_titles_are_localized_and_recursive() {
    let types =
        TypeRegistry::from_definitions([TypeDefinition::new_struct("test.point", "Point", vec![])]);
    with_locale(Locale::Uk, || {
        for (ty, title) in [
            (DataType::Number, "Число"),
            (DataType::String, "Текст"),
            (DataType::Bool, "Так/ні"),
            (DataType::Time, "Час"),
            (DataType::DateTime, "Дата й час"),
            (DataType::Duration, "Тривалість"),
            (DataType::Color, "Колір"),
            (DataType::Object, "Об’єкт"),
            (DataType::Any, "Будь-що"),
        ] {
            assert_eq!(data_type_title(&types, &ty), title);
        }
        assert_eq!(
            data_type_title(&types, &DataType::List(Box::new(DataType::Number))),
            "Список: Число"
        );
        assert_eq!(
            data_type_title(&types, &DataType::Named("test.point".into())),
            "Point"
        );
        assert_eq!(
            data_type_title(&types, &DataType::Named("missing.type".into())),
            "Недоступний тип"
        );
    });
    with_locale(Locale::En, || {
        assert_eq!(
            data_type_title(&types, &DataType::List(Box::new(DataType::String))),
            "List: Text"
        );
    });
    set_runtime_translations(&[("uk".into(), "type-test-point-title = Точка".into())]);
    with_locale(Locale::Uk, || {
        assert_eq!(
            data_type_title(&types, &DataType::Named("test.point".into())),
            "Точка"
        )
    });
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

#[test]
fn property_options_fall_back_to_type_titles_and_unit_symbols() {
    set_known_types(&TypeRegistry::from_definitions(
        quantity::builtin_quantities(),
    ));
    set_runtime_translations(&[
        ("en".into(), "type-quantity-power-title = Power\nunit-kilowatt = kW\n".into()),
        ("uk".into(), "type-quantity-power-title = Потужність\nunit-kilowatt = кВт\nnode-convert-prop-target-option-time = Час доби\nnode-text-to-text-prop-unit-option-auto = Правило проєкту\n".into()),
    ]);
    with_locale(Locale::Uk, || {
        assert_eq!(
            property_option_label("convert", "target", "quantity:quantity.power", "Power"),
            "Потужність"
        );
        assert_eq!(
            property_option_label("other", "type", "quantity.power", "Power"),
            "Потужність"
        );
        assert_eq!(
            property_option_label("convert", "target", "time", "Time of day"),
            "Час доби"
        );
        // Plain words keep the definition label (an HA "bool" option stays "On/Off").
        assert_eq!(
            property_option_label("other", "type", "number", "Number"),
            "Number"
        );
        assert_eq!(
            property_option_label("convert", "unit", "kilowatt", "kW"),
            "кВт"
        );
        assert_eq!(
            property_option_label("text/to_text", "unit", "kilowatt", "kW"),
            "кВт"
        );
        assert_eq!(
            property_option_label("text/to_text", "unit", "auto", "Project rule"),
            "Правило проєкту"
        );
        assert_eq!(
            property_option_label("other", "target", "quantity:missing.type", "Fallback"),
            "Fallback"
        );
        assert_eq!(
            property_option_label("other", "unit", "missing", "Fallback"),
            "Fallback"
        );
    });
    with_locale(Locale::En, || {
        assert_eq!(
            property_option_label("convert", "target", "quantity:quantity.power", "Fallback"),
            "Power"
        );
        assert_eq!(
            property_option_label("convert", "unit", "kilowatt", "Fallback"),
            "kW"
        );
    });
    set_runtime_translations(&[]);
    set_known_types(&TypeRegistry::default());
}
