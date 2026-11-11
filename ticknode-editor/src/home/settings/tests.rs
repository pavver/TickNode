use super::logic::EngineSettingsDraft;
use ticknode_shared::ServerSettings;

#[test]
fn initial_draft_matches_server_and_is_not_dirty() {
    let server = ServerSettings { tick_ms: 100, tick_budget: 0, ..Default::default() };
    let draft = EngineSettingsDraft::new(server.clone());

    assert_eq!(draft.tick_ms, "100");
    assert_eq!(draft.tick_budget, "0");
    assert!(draft.is_valid());
    assert!(!draft.is_dirty(server.clone()));
    assert!(!draft.can_save(server.clone()));
}

#[test]
fn tick_ms_validation_boundaries() {
    let mut draft = EngineSettingsDraft::new(ServerSettings::default());

    draft.tick_ms = "1".into();
    assert_eq!(draft.parse_tick_ms(), Ok(1));
    assert!(draft.is_tick_valid());

    draft.tick_ms = "10000".into();
    assert_eq!(draft.parse_tick_ms(), Ok(10_000));
    assert!(draft.is_tick_valid());

    draft.tick_ms = "0".into();
    assert_eq!(draft.parse_tick_ms(), Err(()));
    assert!(!draft.is_tick_valid());

    draft.tick_ms = "10001".into();
    assert_eq!(draft.parse_tick_ms(), Err(()));
    assert!(!draft.is_tick_valid());

    draft.tick_ms = "abc".into();
    assert_eq!(draft.parse_tick_ms(), Err(()));
    assert!(!draft.is_tick_valid());

    draft.tick_ms = "".into();
    assert_eq!(draft.parse_tick_ms(), Err(()));
    assert!(!draft.is_tick_valid());
}

#[test]
fn budget_validation_boundaries() {
    let mut draft = EngineSettingsDraft::new(ServerSettings::default());

    draft.tick_budget = "0".into();
    assert_eq!(draft.parse_budget(), Ok(0));
    assert!(draft.is_budget_valid());

    draft.tick_budget = "1000000".into();
    assert_eq!(draft.parse_budget(), Ok(1_000_000));
    assert!(draft.is_budget_valid());

    draft.tick_budget = "1000001".into();
    assert_eq!(draft.parse_budget(), Err(()));
    assert!(!draft.is_budget_valid());

    draft.tick_budget = "-1".into();
    assert_eq!(draft.parse_budget(), Err(()));
    assert!(!draft.is_budget_valid());

    draft.tick_budget = "not a number".into();
    assert_eq!(draft.parse_budget(), Err(()));
    assert!(!draft.is_budget_valid());
}

#[test]
fn dirty_detection_and_can_save() {
    let server = ServerSettings { tick_ms: 100, tick_budget: 0, ..Default::default() };
    let mut draft = EngineSettingsDraft::new(server.clone());

    // Initial state: not dirty, cannot save
    assert!(!draft.is_dirty(server.clone()));
    assert!(!draft.can_save(server.clone()));

    // Valid modification
    draft.tick_ms = "50".into();
    assert!(draft.is_dirty(server.clone()));
    assert!(draft.can_save(server.clone()));
    assert_eq!(draft.to_server_settings(), Some(ServerSettings { tick_ms: 50, tick_budget: 0, ..Default::default() }));

    // Invalid modification: dirty, but CANNOT save
    draft.tick_ms = "0".into();
    assert!(draft.is_dirty(server.clone()));
    assert!(!draft.can_save(server.clone()));
    assert_eq!(draft.to_server_settings(), None);

    // Resetting back to server value: not dirty, cannot save
    draft.tick_ms = "100".into();
    assert!(!draft.is_dirty(server.clone()));
    assert!(!draft.can_save(server.clone()));
}

#[test]
fn formatting_locale_is_a_server_setting_in_the_draft() {
    let server = ServerSettings::default();
    let mut draft = EngineSettingsDraft::new(server.clone());
    draft.locale = "uk".into();
    assert!(draft.is_dirty(server.clone()));
    assert!(draft.can_save(server));
    assert_eq!(draft.to_server_settings().unwrap().locale, "uk");
    draft.locale = "de".into();
    assert!(!draft.is_valid());
}

#[test]
fn display_changes_share_engine_save_and_reset_validation() {
    let server = ServerSettings::default();
    let mut draft = EngineSettingsDraft::new(server.clone());
    draft.display.edit("quantity.power", |rule, _| rule.digits = "5".into());
    assert!(draft.is_dirty(server.clone()));
    assert!(draft.can_save(server.clone()));
    assert_eq!(draft.to_server_settings().unwrap().display.quantities["quantity.power"].digits, 5);
    draft.display.edit("quantity.power", |rule, _| rule.digits = "16".into());
    assert!(!draft.can_save(server.clone()));
    assert!(draft.to_server_settings().is_none());
    draft = EngineSettingsDraft::new(server.clone());
    assert!(!draft.is_dirty(server));
}
