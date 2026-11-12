use crate::UiMessage;

/// A syntax error in a Match case or input. `Display` is the English text (logs); `ui_message` gives the
/// translatable form for the user (`error-condition-*` keys in the editor's `ui.ftl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionError {
    MissingPin,
    NotPinNumber(String),
    InputFormat(String),
    InvalidInputName(String),
    UnknownType(String),
    Expected(Expected, Option<String>),
    UnknownOperator(String),
    UnclosedQuote,
    NotNumber(String),
    DoubledOperator(char),
    UnexpectedChar(char),
}

/// The kind of token the parser expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expected {
    Join,
    Operand,
    Operator,
}

impl std::fmt::Display for ConditionError {
    /// The English text is the fallback of the translatable message, so both always agree.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ui_message().fallback)
    }
}

impl std::error::Error for ConditionError {}

impl ConditionError {
    /// The single place that decides each error's key, arguments and English text.
    pub fn ui_message(&self) -> UiMessage {
        match self {
            Self::MissingPin => UiMessage::new(
                "error-condition-missing-pin",
                "missing pin number (\"1: a > 0\")",
            ),
            Self::NotPinNumber(s) => UiMessage::new(
                "error-condition-not-pin-number",
                format!("\"{s}\" is not a pin number"),
            )
            .arg("text", s),
            Self::InputFormat(s) => UiMessage::new(
                "error-condition-input-format",
                format!("\"{s}\": an input is written as \"pin:name:type\""),
            )
            .arg("text", s),
            Self::InvalidInputName(s) => UiMessage::new(
                "error-condition-invalid-input-name",
                format!("\"{s}\" is not a valid input name"),
            )
            .arg("name", s),
            Self::UnknownType(s) => UiMessage::new(
                "error-condition-unknown-type",
                format!("\"{s}\" is an unknown type"),
            )
            .arg("type", s),
            Self::Expected(what, found) => expected_message(*what, found.as_deref()),
            Self::UnknownOperator(s) => UiMessage::new(
                "error-condition-unknown-operator",
                format!("\"{s}\" is an unknown operator"),
            )
            .arg("operator", s),
            Self::UnclosedQuote => {
                UiMessage::new("error-condition-unclosed-quote", "unclosed quote")
            }
            Self::NotNumber(s) => UiMessage::new(
                "error-condition-not-number",
                format!("\"{s}\" is not a number"),
            )
            .arg("text", s),
            Self::DoubledOperator(c) => UiMessage::new(
                "error-condition-doubled-operator",
                format!("\"{c}\" — probably \"{c}{c}\""),
            )
            .arg("char", c.to_string()),
            Self::UnexpectedChar(c) => UiMessage::new(
                "error-condition-unexpected-char",
                format!("unexpected character \"{c}\""),
            )
            .arg("char", c.to_string()),
        }
    }
}

/// `end` selects the wording in Fluent; `found` is the offending token.
fn expected_message(what: Expected, found: Option<&str>) -> UiMessage {
    let description = what.description();
    let text = match found {
        Some(s) => format!("expected {description}, not «{s}»"),
        None => format!("expected {description}, not end of text"),
    };
    UiMessage::new(what.key(), text)
        .arg("end", found.is_none().to_string())
        .arg("found", found.unwrap_or_default())
}

impl Expected {
    fn description(self) -> &'static str {
        match self {
            Self::Join => "\"and\" / \"or\"",
            Self::Operand => "an operand",
            Self::Operator => "an operator",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Join => "error-condition-expected-join",
            Self::Operand => "error-condition-expected-operand",
            Self::Operator => "error-condition-expected-operator",
        }
    }
}
