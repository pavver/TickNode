use super::*;

#[test]
fn default_settings() {
    let settings = ServerSettings::default();
    assert_eq!(settings.tick_ms, 100);
    assert_eq!(settings.tick_budget, 0);
}

#[test]
fn validation_clamps_values() {
    let under = ServerSettings {
        tick_ms: 0,
        tick_budget: 0,
        ..Default::default()
    }
    .validated();
    assert_eq!(under.tick_ms, 1);
    assert_eq!(under.tick_budget, 0);

    let over = ServerSettings {
        tick_ms: 20_000,
        tick_budget: 5_000_000,
        ..Default::default()
    }
    .validated();
    assert_eq!(over.tick_ms, 10_000);
    assert_eq!(over.tick_budget, 1_000_000);

    let valid = ServerSettings {
        tick_ms: 50,
        tick_budget: 500,
        ..Default::default()
    }
    .validated();
    assert_eq!(valid.tick_ms, 50);
    assert_eq!(valid.tick_budget, 500);
}

#[test]
fn locale_defaults_and_is_validated_without_changing_postcard_field_order() {
    let legacy: ServerSettings =
        serde_json::from_str(r#"{"tick_ms":50,"tick_budget":10}"#).unwrap();
    assert_eq!(legacy.locale, "en");
    for input in ["en", "uk", "de", "", "UK"] {
        let settings = ServerSettings {
            locale: input.into(),
            ..Default::default()
        }
        .validated();
        assert_eq!(settings.locale, if input == "uk" { "uk" } else { "en" });
        let bytes = postcard::to_allocvec(&settings).unwrap();
        assert_eq!(
            postcard::from_bytes::<ServerSettings>(&bytes).unwrap(),
            settings
        );
        assert_eq!(
            serde_json::from_str::<ServerSettings>(&serde_json::to_string(&settings).unwrap())
                .unwrap(),
            settings
        );
    }
}
