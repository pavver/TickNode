use super::*;

#[test]
fn test_auto_close_open_paren() {
    let source = "x + ";
    let action = decide_typing(source, 4..4, "(", None);
    assert_eq!(
        action,
        AutoCloseAction::Insert {
            range: 4..4,
            text: "()",
            new_caret: 5,
            state: AutoCloseState { start: 4, caret: 5 },
        }
    );
}

#[test]
fn test_skip_closing_paren() {
    let source = "(x)";
    let action = decide_typing(source, 2..2, ")", None);
    assert_eq!(action, AutoCloseAction::SkipChar { new_caret: 3 });
}

#[test]
fn test_immediate_backspace_deletes_pair() {
    let source = "()";
    let state = AutoCloseState { start: 0, caret: 1 };
    let action = decide_typing(source, 1..1, "Backspace", Some(state));
    assert_eq!(
        action,
        AutoCloseAction::DeletePair {
            range: 0..2,
            new_caret: 0,
        }
    );
}

#[test]
fn test_wrap_selection() {
    let source = "a + b";
    let action = decide_typing(source, 0..5, "(", None);
    assert_eq!(
        action,
        AutoCloseAction::Wrap {
            range: 0..5,
            new_text: "(a + b)".to_string(),
            new_caret: 7,
        }
    );
}
