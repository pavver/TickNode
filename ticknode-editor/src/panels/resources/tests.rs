use std::collections::HashMap;
use std::sync::Arc;
use ticknode_shared::{PropertyOption, ResourceAction};
use crate::panels::resources::available_actions;
use crate::state::PropertyOptions;

fn opt(val: &str) -> PropertyOption {
    PropertyOption {
        value: val.to_string(),
        label: val.to_string(),
        detail: String::new(),
        icon: None,
    }
}

#[test]
fn test_available_actions_unconditional() {
    let actions = vec![
        ResourceAction::new("Trigger", "ha/trigger_state", "entity_id"),
        ResourceAction::new("Value", "ha/entity", "entity_id"),
    ];
    let options: PropertyOptions = HashMap::new();
    let result = available_actions(&actions, "home_assistant", "light.kitchen", &options);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].type_name, "ha/trigger_state");
    assert_eq!(result[1].type_name, "ha/entity");
}

#[test]
fn test_available_actions_requires_source_satisfied() {
    let actions = vec![
        ResourceAction::new("Trigger", "ha/trigger_state", "entity_id"),
        ResourceAction::new("Action", "ha/action", "target").requires_source("action_targets"),
    ];
    let mut options: PropertyOptions = HashMap::new();
    options.insert(
        ("home_assistant".to_string(), "action_targets".to_string()),
        Arc::new(vec![opt("light.kitchen"), opt("switch.fan")]),
    );

    let kept = available_actions(&actions, "home_assistant", "light.kitchen", &options);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[1].type_name, "ha/action");
}

#[test]
fn test_available_actions_requires_source_not_satisfied() {
    let actions = vec![
        ResourceAction::new("Trigger", "ha/trigger_state", "entity_id"),
        ResourceAction::new("Action", "ha/action", "target").requires_source("action_targets"),
    ];
    let mut options: PropertyOptions = HashMap::new();
    options.insert(
        ("home_assistant".to_string(), "action_targets".to_string()),
        Arc::new(vec![opt("switch.fan")]),
    );

    let filtered = available_actions(&actions, "home_assistant", "sensor.temp", &options);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].type_name, "ha/trigger_state");
}

#[test]
fn test_available_actions_missing_source() {
    let actions = vec![
        ResourceAction::new("Action", "ha/action", "target").requires_source("missing_source"),
    ];
    let options: PropertyOptions = HashMap::new();
    let filtered = available_actions(&actions, "home_assistant", "light.kitchen", &options);
    assert!(filtered.is_empty());
}

#[test]
fn test_available_actions_different_plugin() {
    let actions = vec![
        ResourceAction::new("Action", "ha/action", "target").requires_source("action_targets"),
    ];
    let mut options: PropertyOptions = HashMap::new();
    options.insert(
        ("other_plugin".to_string(), "action_targets".to_string()),
        Arc::new(vec![opt("light.kitchen")]),
    );

    let filtered = available_actions(&actions, "home_assistant", "light.kitchen", &options);
    assert!(filtered.is_empty());
}
