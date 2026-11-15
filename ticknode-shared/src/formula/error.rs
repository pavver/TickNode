use crate::UiMessage;
use std::ops::Range;

/// Structured formula error containing a UI message (key + args + fallback) and source byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaError {
    pub message: UiMessage,
    pub range: Range<usize>,
}

impl FormulaError {
    pub fn new(range: Range<usize>, key: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self {
            message: UiMessage::new(key, fallback),
            range,
        }
    }

    pub fn with_arg(mut self, name: &str, value: impl ToString) -> Self {
        self.message = self.message.arg(name, value);
        self
    }

    pub fn too_large(len: usize, max: usize) -> Self {
        Self::new(
            0..len,
            "error-formula-too-large",
            format!("Formula source length ({len} bytes) exceeds limit of {max} bytes"),
        )
        .with_arg("len", len)
        .with_arg("max", max)
    }

    pub fn depth_exceeded(range: Range<usize>, max: usize) -> Self {
        Self::new(
            range,
            "error-formula-depth-exceeded",
            format!("Formula nesting depth exceeds limit of {max}"),
        )
        .with_arg("max", max)
    }

    pub fn unexpected_token(range: Range<usize>, token: &str) -> Self {
        Self::new(
            range,
            "error-formula-unexpected-token",
            format!("Unexpected token '{token}'"),
        )
        .with_arg("token", token)
    }

    pub fn unclosed_paren(range: Range<usize>) -> Self {
        Self::new(
            range,
            "error-formula-unclosed-paren",
            "Unclosed parenthesis in formula",
        )
    }

    pub fn unknown_function(range: Range<usize>, name: &str) -> Self {
        Self::new(
            range,
            "error-formula-unknown-function",
            format!("Unknown function '{name}'"),
        )
        .with_arg("name", name)
    }

    pub fn unknown_function_suggest(range: Range<usize>, name: &str, suggestion: &str) -> Self {
        Self::new(
            range,
            "error-formula-unknown-function-suggest",
            format!("Unknown function '{name}'. Did you mean '{suggestion}'?"),
        )
        .with_arg("name", name)
        .with_arg("suggestion", suggestion)
    }

    pub fn wrong_arity(range: Range<usize>, name: &str, expected: &str, got: usize) -> Self {
        Self::new(
            range,
            "error-formula-wrong-arity",
            format!("Function '{name}' expects {expected} arguments, got {got}"),
        )
        .with_arg("name", name)
        .with_arg("expected", expected)
        .with_arg("got", got)
    }

    pub fn reserved_name(range: Range<usize>, name: &str) -> Self {
        Self::new(
            range,
            "error-formula-reserved-name",
            format!("Variable name '{name}' is reserved"),
        )
        .with_arg("name", name)
    }

    pub fn division_by_zero(range: Range<usize>) -> Self {
        Self::new(
            range,
            "error-formula-division-by-zero",
            "Division by zero in formula",
        )
    }

    pub fn domain_error(range: Range<usize>, name: &str) -> Self {
        Self::new(
            range,
            "error-formula-domain-error",
            format!("Mathematical domain error in function '{name}'"),
        )
        .with_arg("name", name)
    }

    pub fn overflow(range: Range<usize>) -> Self {
        Self::new(
            range,
            "error-formula-overflow",
            "Arithmetic overflow in formula",
        )
    }

    pub fn incompatible_quantities(range: Range<usize>, a: &str, b: &str) -> Self {
        Self::new(
            range,
            "error-formula-incompatible-quantities",
            format!("Incompatible physical quantities: cannot combine {a} and {b}"),
        )
        .with_arg("a", a)
        .with_arg("b", b)
    }

    pub fn type_mismatch(range: Range<usize>, expected: &str, got: &str) -> Self {
        Self::new(
            range,
            "error-formula-type-mismatch",
            format!("Type mismatch: expected {expected}, got {got}"),
        )
        .with_arg("expected", expected)
        .with_arg("got", got)
    }
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FormulaError {}
