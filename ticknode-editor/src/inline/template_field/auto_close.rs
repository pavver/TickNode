//! Pure decision logic for auto-closing delimiters and pair management in the template editor.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosePairKind {
    Interpolation, // {{ | }}
    Tag,           // {% | %}
    Comment,       // {# | #}
}

impl AutoClosePairKind {
    pub fn full_empty_pair(&self) -> &'static str {
        match self {
            Self::Interpolation => "{{  }}",
            Self::Tag => "{%  %}",
            Self::Comment => "{#  #}",
        }
    }
}

/// Recorded state after an auto-closing insertion to detect immediate backspace deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoCloseState {
    pub kind: AutoClosePairKind,
    /// Byte offset where the opening `{` started in `source`.
    pub start: usize,
    /// Byte offset of the caret immediately after the auto-insert.
    pub caret: usize,
    /// Byte length of the entire inserted pair construct (e.g. 6 for `{{  }}`).
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoCloseAction {
    /// No auto-closing intercepted; allow normal typing.
    None,
    /// Insert text at `range` and move caret to `new_caret`.
    Insert {
        range: Range<usize>,
        text: &'static str,
        new_caret: usize,
        state: AutoCloseState,
    },
    /// Move the caret over the next character without inserting anything.
    SkipChar { new_caret: usize },
    /// Delete the auto-inserted pair on immediate backspace.
    DeletePair {
        range: Range<usize>,
        replacement: &'static str,
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
    if selection.start != selection.end {
        return AutoCloseAction::None;
    }
    let caret = selection.start.min(source.len());

    if key == "Backspace" {
        if let Some(state) = last_auto_close
            && state.caret == caret
            && state.start + state.len <= source.len()
            && &source[state.start..state.start + state.len] == state.kind.full_empty_pair()
        {
            return AutoCloseAction::DeletePair {
                range: state.start..state.start + state.len,
                replacement: "{",
                new_caret: state.start + 1,
            };
        }
        return AutoCloseAction::None;
    }

    if key == "{" {
        if source[..caret].ends_with('{') {
            let after = &source[caret..];
            if !after.trim_start().starts_with("}}") {
                let state = AutoCloseState {
                    kind: AutoClosePairKind::Interpolation,
                    start: caret - 1,
                    caret: caret + 2,
                    len: 6,
                };
                return AutoCloseAction::Insert {
                    range: caret..caret,
                    text: "{  }}",
                    new_caret: caret + 2,
                    state,
                };
            }
        }
        return AutoCloseAction::None;
    }

    if key == "%" {
        if source[..caret].ends_with('{') {
            let after = &source[caret..];
            if !after.trim_start().starts_with("%}") {
                let state = AutoCloseState {
                    kind: AutoClosePairKind::Tag,
                    start: caret - 1,
                    caret: caret + 2,
                    len: 6,
                };
                return AutoCloseAction::Insert {
                    range: caret..caret,
                    text: "%  %}",
                    new_caret: caret + 2,
                    state,
                };
            }
        }
        return AutoCloseAction::None;
    }

    if key == "#" {
        if source[..caret].ends_with('{') {
            let after = &source[caret..];
            if !after.trim_start().starts_with("#}") {
                let state = AutoCloseState {
                    kind: AutoClosePairKind::Comment,
                    start: caret - 1,
                    caret: caret + 2,
                    len: 6,
                };
                return AutoCloseAction::Insert {
                    range: caret..caret,
                    text: "#  #}",
                    new_caret: caret + 2,
                    state,
                };
            }
        }
        return AutoCloseAction::None;
    }

    if key == "}" {
        let after = &source[caret..];
        if after.starts_with('}') {
            let before = &source[..caret];
            let is_closer = after.starts_with("}}")
                || before.ends_with('}')
                || before.ends_with('%')
                || before.ends_with('#');
            if is_closer {
                return AutoCloseAction::SkipChar {
                    new_caret: caret + 1,
                };
            }
        }
        return AutoCloseAction::None;
    }

    AutoCloseAction::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_interpolation() {
        let source = "Hello {";
        let action = decide_typing(source, 7..7, "{", None);
        assert_eq!(
            action,
            AutoCloseAction::Insert {
                range: 7..7,
                text: "{  }}",
                new_caret: 9,
                state: AutoCloseState {
                    kind: AutoClosePairKind::Interpolation,
                    start: 6,
                    caret: 9,
                    len: 6,
                }
            }
        );
    }

    #[test]
    fn test_open_interpolation_suppressed_when_already_closed() {
        let source = "Hello { }}";
        let action = decide_typing(source, 7..7, "{", None);
        assert_eq!(action, AutoCloseAction::None);

        let source2 = "Hello {}}";
        let action2 = decide_typing(source2, 7..7, "{", None);
        assert_eq!(action2, AutoCloseAction::None);
    }

    #[test]
    fn test_open_tag() {
        let source = "Hello {";
        let action = decide_typing(source, 7..7, "%", None);
        assert_eq!(
            action,
            AutoCloseAction::Insert {
                range: 7..7,
                text: "%  %}",
                new_caret: 9,
                state: AutoCloseState {
                    kind: AutoClosePairKind::Tag,
                    start: 6,
                    caret: 9,
                    len: 6,
                }
            }
        );
    }

    #[test]
    fn test_open_tag_suppressed_when_already_closed() {
        let source = "Hello { %}";
        let action = decide_typing(source, 7..7, "%", None);
        assert_eq!(action, AutoCloseAction::None);
    }

    #[test]
    fn test_open_comment() {
        let source = "Hello {";
        let action = decide_typing(source, 7..7, "#", None);
        assert_eq!(
            action,
            AutoCloseAction::Insert {
                range: 7..7,
                text: "#  #}",
                new_caret: 9,
                state: AutoCloseState {
                    kind: AutoClosePairKind::Comment,
                    start: 6,
                    caret: 9,
                    len: 6,
                }
            }
        );
    }

    #[test]
    fn test_skip_char_closing_braces() {
        // Right before `}}`
        let source = "{{ value }}";
        // Caret at index 9: right before the first `}`
        let action = decide_typing(source, 9..9, "}", None);
        assert_eq!(action, AutoCloseAction::SkipChar { new_caret: 10 });

        // Caret at index 10: right before the second `}`
        let action2 = decide_typing(source, 10..10, "}", None);
        assert_eq!(action2, AutoCloseAction::SkipChar { new_caret: 11 });

        // In tag: `{% value %}` before `}`
        let tag_source = "{% value %}";
        let tag_action = decide_typing(tag_source, 10..10, "}", None);
        assert_eq!(tag_action, AutoCloseAction::SkipChar { new_caret: 11 });

        // In comment: `{# comment #}` before `}`
        let cmt_source = "{# comment #}";
        let cmt_action = decide_typing(cmt_source, 12..12, "}", None);
        assert_eq!(cmt_action, AutoCloseAction::SkipChar { new_caret: 13 });

        // Isolated `}` in plain text without closer context
        let plain = "value }";
        let plain_action = decide_typing(plain, 6..6, "}", None);
        assert_eq!(plain_action, AutoCloseAction::None);
    }

    #[test]
    fn test_immediate_backspace_deletes_pair() {
        let source = "{{  }}";
        let state = AutoCloseState {
            kind: AutoClosePairKind::Interpolation,
            start: 0,
            caret: 3,
            len: 6,
        };
        let action = decide_typing(source, 3..3, "Backspace", Some(state));
        assert_eq!(
            action,
            AutoCloseAction::DeletePair {
                range: 0..6,
                replacement: "{",
                new_caret: 1,
            }
        );
    }

    #[test]
    fn test_backspace_after_typing_does_normal_backspace() {
        let source = "{{ x }}";
        let state = AutoCloseState {
            kind: AutoClosePairKind::Interpolation,
            start: 0,
            caret: 3,
            len: 6,
        };
        // Caret moved to 4 because 'x' was typed
        let action = decide_typing(source, 4..4, "Backspace", Some(state));
        assert_eq!(action, AutoCloseAction::None);

        // Or without state (state cleared on typing)
        let action2 = decide_typing(source, 3..3, "Backspace", None);
        assert_eq!(action2, AutoCloseAction::None);
    }
}
