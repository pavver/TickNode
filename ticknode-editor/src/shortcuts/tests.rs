use super::*;

#[test]
fn shortcuts_keep_both_keyboard_layouts_and_default_handling() {
    for (ctrl, keys, action) in [
        (false, &["Delete", "Backspace"][..], Shortcut::Delete),
        (false, &["Escape"][..], Shortcut::Escape),
        (false, &["f", "F", "а", "А"][..], Shortcut::Fit),
        (false, &["c", "C", "с", "С"][..], Shortcut::CreateFrame),
        (true, &["z", "Z", "я", "Я"][..], Shortcut::Undo),
        (true, &["y", "Y", "н", "Н"][..], Shortcut::Redo),
        (true, &["a", "A", "ф", "Ф"][..], Shortcut::SelectAll),
        (true, &["d", "D", "в", "В"][..], Shortcut::Duplicate),
    ] {
        for key in keys {
            let selected = shortcut(ctrl, false, key, false).unwrap();
            assert_eq!(selected, action);
            assert_eq!(selected.prevents_default(), ctrl);
        }
    }
    assert_eq!(shortcut(true, false, "v", false), None);
    assert_eq!(shortcut(false, false, "a", false), None);
    assert_eq!(shortcut(true, false, "Delete", false), None);
}

#[test]
fn paste_blocks_history_but_leaves_other_shortcuts_available() {
    for key in ["z", "я", "y", "н"] {
        assert_eq!(shortcut(true, false, key, true), None);
        assert_eq!(shortcut(true, true, key, true), None);
        assert_eq!(shortcut(true, true, key, false), Some(Shortcut::Redo));
    }
    assert_eq!(shortcut(true, false, "d", true), Some(Shortcut::Duplicate));
    assert_eq!(
        shortcut(false, false, "Escape", true),
        Some(Shortcut::Escape)
    );
    assert_eq!(shortcut(false, false, "c", true), None);
    assert_eq!(shortcut(false, true, "f", false), Some(Shortcut::Fit));
    assert_eq!(
        shortcut(false, true, "c", false),
        Some(Shortcut::CreateFrame)
    );
}
