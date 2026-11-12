use super::*;
use crate::i18n;

#[test]
fn navigation_clamps_and_keeps_empty_list_key_handling() {
    assert_eq!(key_action("ArrowDown", 0, 3), KeyAction::Move(1));
    assert_eq!(key_action("ArrowDown", 2, 3), KeyAction::Move(2));
    assert_eq!(key_action("ArrowDown", 8, 3), KeyAction::Move(2));
    assert_eq!(key_action("ArrowDown", 0, 0), KeyAction::Handled);
    assert_eq!(key_action("ArrowUp", 0, 0), KeyAction::Move(0));
    assert_eq!(key_action("ArrowUp", 3, 0), KeyAction::Move(2));
    assert_eq!(key_action("Enter", 0, 0), KeyAction::Pick);
    assert_eq!(key_action("Escape", 0, 0), KeyAction::Close);
    assert_eq!(key_action("a", 0, 0), KeyAction::Ignore);
}

#[test]
fn enter_prefers_the_selected_option_then_trimmed_text() {
    let options = [PropertyOption {
        value: "sensor.room".into(),
        label: "Room".into(),
        detail: String::new(),
        icon: None,
    }];
    assert_eq!(
        chosen_value(&options, Some(0), " typed "),
        Some("sensor.room".into())
    );
    assert_eq!(
        chosen_value(&options, None, " typed "),
        Some("typed".into())
    );
    assert_eq!(chosen_value(&options, None, "  "), None);
    assert!(option_is_current("sensor.room", "sensor.room"));
    assert!(option_is_current("", ""));
    assert!(!option_is_current("sensor.room", "sensor.other"));
}

#[test]
fn notes_distinguish_an_empty_source_a_filter_and_truncated_results() {
    i18n::with_locale(i18n::Locale::En, || {
        assert_eq!(list_note(true, true, 0, 0), Some(t!("picker-no-options")));
        assert_eq!(list_note(false, true, 0, 0), Some(t!("picker-empty")));
        assert_eq!(
            list_note(false, false, 100, 125),
            Some(t!("picker-more", count = 25))
        );
        assert_eq!(list_note(true, false, 0, 0), None);
        assert_eq!(list_note(false, false, 1, 1), None);
    });
}

#[test]
fn input_mode_is_first_selectable_row_and_can_be_restored_from_a_chosen_entity() {
    let options = [PropertyOption {
        value: "light.hall".into(),
        label: "Hall".into(),
        detail: String::new(),
        icon: None,
    }];
    let options = input_options(&options, "From input");
    assert_eq!(options.len(), 2);
    assert_eq!(filter_options(&options, "", MAX_ROWS).0, vec![0, 1]);
    assert_eq!(chosen_value(&options, Some(0), ""), Some(String::new()));
    assert!(option_is_current("", &options[0].value));
    assert!(!option_is_current("light.hall", &options[0].value));
    assert_eq!(
        chosen_value(&options, Some(1), ""),
        Some("light.hall".into())
    );
    assert_eq!(filter_options(&options, "hall", MAX_ROWS).0, vec![1]);
}
