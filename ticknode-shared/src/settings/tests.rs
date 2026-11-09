use super::*;

#[test]
fn default_settings() {
    let settings = ServerSettings::default();
    assert_eq!(settings.tick_ms, 100);
    assert_eq!(settings.tick_budget, 0);
}

#[test]
fn validation_clamps_values() {
    let under = ServerSettings { tick_ms: 0, tick_budget: 0 }.validated();
    assert_eq!(under.tick_ms, 1);
    assert_eq!(under.tick_budget, 0);

    let over = ServerSettings { tick_ms: 20_000, tick_budget: 5_000_000 }.validated();
    assert_eq!(over.tick_ms, 10_000);
    assert_eq!(over.tick_budget, 1_000_000);

    let valid = ServerSettings { tick_ms: 50, tick_budget: 500 }.validated();
    assert_eq!(valid.tick_ms, 50);
    assert_eq!(valid.tick_budget, 500);
}
