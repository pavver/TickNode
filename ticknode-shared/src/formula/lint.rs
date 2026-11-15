//! Type-aware and semantic linting for mathematical formulas.

use super::catalog::{CATALOG, find_function};
use crate::DataType;
use crate::UiMessage;
pub use crate::template::lint::{Lint, LintSeverity, find_suggestion};
use std::collections::HashMap;
use std::ops::Range;

#[cfg(test)]
mod tests;

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b':'
}

struct CallSpan {
    name: String,
    name_range: Range<usize>,
    call_range: Range<usize>,
    arg_count: usize,
}

fn scan_calls(source: &str) -> Vec<CallSpan> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut calls = Vec::new();

    while i < len {
        if is_ident_char(bytes[i]) {
            let start = i;
            while i < len && is_ident_char(bytes[i]) {
                i += 1;
            }
            let name = &source[start..i];
            let name_range = start..i;

            let mut next_k = i;
            while next_k < len && bytes[next_k].is_ascii_whitespace() {
                next_k += 1;
            }
            if next_k < len && bytes[next_k] == b'(' {
                let lparen_pos = next_k;
                let mut pos = lparen_pos + 1;
                let mut depth = 1;
                let mut args = 0;
                let mut has_token_in_arg = false;

                while pos < len && depth > 0 {
                    let b = bytes[pos];
                    if b == b'(' {
                        depth += 1;
                        has_token_in_arg = true;
                    } else if b == b')' {
                        depth -= 1;
                        if depth == 0 {
                            if has_token_in_arg {
                                args += 1;
                            }
                            break;
                        }
                    } else if b == b',' && depth == 1 {
                        args += 1;
                        has_token_in_arg = false;
                    } else if !b.is_ascii_whitespace() {
                        has_token_in_arg = true;
                    }
                    pos += 1;
                }

                let call_end = if pos < len { pos + 1 } else { pos };
                calls.push(CallSpan {
                    name: name.to_string(),
                    name_range,
                    call_range: start..call_end,
                    arg_count: args,
                });
                i = pos;
                continue;
            }
        }
        i += 1;
    }

    calls
}

/// Analyzes a formula for unknown functions, arity errors, and physical quantity inference mismatches.
pub fn lint(source: &str, inputs: &[(String, DataType)]) -> Vec<Lint> {
    let mut lints = Vec::new();

    // 1. Scan calls for unknown functions and arity
    let calls = scan_calls(source);
    for call in calls {
        if let Some(doc) = find_function(&call.name) {
            let got = call.arg_count;
            if got < doc.min_args || doc.max_args.is_some_and(|max| got > max) {
                let expected = match doc.max_args {
                    Some(max) if max == doc.min_args => format!("{}", doc.min_args),
                    Some(max) => format!("{}-{}", doc.min_args, max),
                    None => format!("at least {}", doc.min_args),
                };
                lints.push(Lint {
                    range: call.call_range,
                    message: UiMessage::new(
                        "formula-lint-wrong-arity",
                        format!(
                            "Function '{}' expects {expected} arguments, got {got}",
                            doc.name
                        ),
                    )
                    .arg("name", doc.name)
                    .arg("expected", expected)
                    .arg("got", got),
                    severity: LintSeverity::Warning,
                });
            }
        } else {
            let sugg = find_suggestion(&call.name, CATALOG.iter().map(|d| d.name));
            if let Some(s) = sugg {
                lints.push(Lint {
                    range: call.name_range,
                    message: UiMessage::new(
                        "formula-lint-unknown-function-suggest",
                        format!("Unknown function '{}'. Did you mean '{s}'?", call.name),
                    )
                    .arg("name", call.name)
                    .arg("suggestion", s),
                    severity: LintSeverity::Warning,
                });
            } else {
                lints.push(Lint {
                    range: call.name_range,
                    message: UiMessage::new(
                        "formula-lint-unknown-function",
                        format!("Unknown function '{}'", call.name),
                    )
                    .arg("name", call.name),
                    severity: LintSeverity::Warning,
                });
            }
        }
    }

    // 2. Type inference on valid AST to detect quantity mismatches
    if let Ok(compiled) = super::compile(source) {
        let input_types: HashMap<String, DataType> = inputs.iter().cloned().collect();
        if let Err(err) = compiled.infer_type(&input_types) {
            lints.push(Lint {
                range: err.range,
                message: err.message,
                severity: LintSeverity::Warning,
            });
        }
    }

    lints
}
