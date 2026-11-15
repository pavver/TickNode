//! Pure decision logic for auto-closing parentheses in the formula editor.

use std::ops::Range;

#[cfg(test)]
mod tests;

/// Recorded state after an auto-closing insertion to detect immediate backspace deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoCloseState {
    pub start: usize,
    pub caret: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoCloseAction {
    None,
    Insert {
        range: Range<usize>,
        text: &'static str,
        new_caret: usize,
        state: AutoCloseState,
    },
    Wrap {
        range: Range<usize>,
        new_text: String,
        new_caret: usize,
    },
    SkipChar {
        new_caret: usize,
    },
    DeletePair {
        range: Range<usize>,
        new_caret: usize,
    },
}

/// Pure decision function evaluating whether a keystroke triggers auto-closing, closer skipping,
/// or pair deletion on backspace.
pub fn decide_typing(
    source: &str,
    selection: Range<usize>,
    key: &str,
    last_auto_close: Option<AutoCloseState>,
) -> AutoCloseAction {
    // If text is selected and user presses '(', wrap selection in (...)
    if selection.start != selection.end {
        if key == "(" {
            let start = selection.start.min(source.len());
            let end = selection.end.min(source.len());
            let selected_slice = &source[start..end];
            let wrapped = format!("({selected_slice})");
            return AutoCloseAction::Wrap {
                range: start..end,
                new_text: wrapped,
                new_caret: end + 2,
            };
        }
        return AutoCloseAction::None;
    }

    let caret = selection.start.min(source.len());

    if key == "Backspace" {
        if let Some(state) = last_auto_close
            && state.caret == caret
            && state.start + 2 <= source.len()
            && &source[state.start..state.start + 2] == "()"
        {
            return AutoCloseAction::DeletePair {
                range: state.start..state.start + 2,
                new_caret: state.start,
            };
        }
        return AutoCloseAction::None;
    }

    if key == "(" {
        let state = AutoCloseState {
            start: caret,
            caret: caret + 1,
        };
        return AutoCloseAction::Insert {
            range: caret..caret,
            text: "()",
            new_caret: caret + 1,
            state,
        };
    }

    if key == ")" {
        let after = &source[caret..];
        if after.starts_with(')') {
            return AutoCloseAction::SkipChar {
                new_caret: caret + 1,
            };
        }
        return AutoCloseAction::None;
    }

    AutoCloseAction::None
}
