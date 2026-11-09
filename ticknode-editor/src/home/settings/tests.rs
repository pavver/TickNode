use super::logic::EngineSettingsDraft;
use ticknode_shared::ServerSettings;

#[test]
fn initial_draft_matches_server_and_is_not_dirty() {
    let server = ServerSettings { tick_ms: 100, tick_budget: 0 };
    let draft = EngineSettingsDraft::new(server);

    assert_eq!(draft.tick_ms, "100");
    assert_eq!(draft.tick_budget, "0");
    assert!(draft.is_valid());
    assert!(!draft.is_dirty(server));
    assert!(!draft.can_save(server));
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
    let server = ServerSettings { tick_ms: 100, tick_budget: 0 };
    let mut draft = EngineSettingsDraft::new(server);

    // Initial state: not dirty, cannot save
    assert!(!draft.is_dirty(server));
    assert!(!draft.can_save(server));

    // Valid modification
    draft.tick_ms = "50".into();
    assert!(draft.is_dirty(server));
    assert!(draft.can_save(server));
    assert_eq!(draft.to_server_settings(), Some(ServerSettings { tick_ms: 50, tick_budget: 0 }));

    // Invalid modification: dirty, but CANNOT save
    draft.tick_ms = "0".into();
    assert!(draft.is_dirty(server));
    assert!(!draft.can_save(server));
    assert_eq!(draft.to_server_settings(), None);

    // Resetting back to server value: not dirty, cannot save
    draft.tick_ms = "100".into();
    assert!(!draft.is_dirty(server));
    assert!(!draft.can_save(server));
}
