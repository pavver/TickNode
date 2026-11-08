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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPin => write!(f, "missing pin number (\"1: a > 0\")"),
            Self::NotPinNumber(s) => write!(f, "\"{s}\" is not a pin number"),
            Self::InputFormat(s) => write!(f, "\"{s}\": an input is written as \"pin:name:type\""),
            Self::InvalidInputName(s) => write!(f, "\"{s}\" is not a valid input name"),
            Self::UnknownType(s) => write!(f, "\"{s}\" is an unknown type"),
            Self::Expected(what, found) => {
                let what = match what {
                    Expected::Join => "\"and\" / \"or\"",
                    Expected::Operand => "an operand",
                    Expected::Operator => "an operator",
                };
                match found {
                    Some(s) => write!(f, "expected {what}, not «{s}»"),
                    None => write!(f, "expected {what}, not end of text"),
                }
            }
            Self::UnknownOperator(s) => write!(f, "\"{s}\" is an unknown operator"),
            Self::UnclosedQuote => write!(f, "unclosed quote"),
            Self::NotNumber(s) => write!(f, "\"{s}\" is not a number"),
            Self::DoubledOperator(c) => write!(f, "\"{c}\" — probably \"{c}{c}\""),
            Self::UnexpectedChar(c) => write!(f, "unexpected character \"{c}\""),
        }
    }
}

impl std::error::Error for ConditionError {}

impl ConditionError {
    pub fn ui_message(&self) -> UiMessage {
        let text = self.to_string();
        match self {
            Self::MissingPin => UiMessage::new("error-condition-missing-pin", text),
            Self::NotPinNumber(s) => UiMessage::new("error-condition-not-pin-number", text).arg("text", s),
            Self::InputFormat(s) => UiMessage::new("error-condition-input-format", text).arg("text", s),
            Self::InvalidInputName(s) => UiMessage::new("error-condition-invalid-input-name", text).arg("name", s),
            Self::UnknownType(s) => UiMessage::new("error-condition-unknown-type", text).arg("type", s),
            Self::Expected(what, found) => {
                let key = match what {
                    Expected::Join => "error-condition-expected-join",
                    Expected::Operand => "error-condition-expected-operand",
                    Expected::Operator => "error-condition-expected-operator",
                };
                // `end` selects the wording in Fluent; `found` is the offending token
                UiMessage::new(key, text).arg("end", found.is_none().to_string()).arg("found", found.clone().unwrap_or_default())
            }
            Self::UnknownOperator(s) => UiMessage::new("error-condition-unknown-operator", text).arg("operator", s),
            Self::UnclosedQuote => UiMessage::new("error-condition-unclosed-quote", text),
            Self::NotNumber(s) => UiMessage::new("error-condition-not-number", text).arg("text", s),
            Self::DoubledOperator(c) => UiMessage::new("error-condition-doubled-operator", text).arg("char", c.to_string()),
            Self::UnexpectedChar(c) => UiMessage::new("error-condition-unexpected-char", text).arg("char", c.to_string()),
        }
    }
}
