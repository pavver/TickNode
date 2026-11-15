//! Type-aware and semantic linting for MiniJinja templates.

use crate::template::complete::catalog::{BUILTIN_FILTERS, TESTS, filter_accepts};
use crate::template::complete::infer::{InferContext, infer_left};
use crate::template::complete::kinds::{ValueKind, kind_label};
use crate::template::complete::scan::{ForScope, process_closed_tag, scan_prefix};
use crate::types::TypeKind;
use crate::{DataType, TypeRegistry, UiMessage};
use minijinja::machinery::{Token, tokenize};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[cfg(test)]
mod tests;

/// Severity of a template lint issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintSeverity {
    Warning,
}

/// A diagnostic message reported at a source range within a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lint {
    pub range: Range<usize>,
    pub message: UiMessage,
    pub severity: LintSeverity,
}

/// Computes the Levenshtein edit distance between two character sequences.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = (0..=n).collect::<Vec<usize>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Finds the closest candidate with edit distance <= 2, breaking ties alphabetically.
pub fn find_suggestion<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let mut best: Option<(&'a str, usize)> = None;
    for cand in candidates {
        let dist = edit_distance(target, cand);
        if dist <= 2 {
            match best {
                Some((best_cand, best_dist)) => {
                    if dist < best_dist || (dist == best_dist && cand < best_cand) {
                        best = Some((cand, dist));
                    }
                }
                None => {
                    best = Some((cand, dist));
                }
            }
        }
    }
    best.map(|(s, _)| s.to_string())
}

/// Returns true if `name` is a known filter (built-in or TickNode filter).
pub fn is_known_filter(name: &str) -> bool {
    name == "text" || BUILTIN_FILTERS.iter().any(|(n, _)| *n == name)
}

/// Analyzes a template for type-aware and semantic issues.
pub fn lint(source: &str, inputs: &[(String, DataType)], types: &TypeRegistry) -> Vec<Lint> {
    lint_with_variables(source, inputs, types, &[])
}

/// Analyzes a template for type-aware and semantic issues, including visible variables.
pub fn lint_with_variables(
    source: &str,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
    variables: &[crate::Variable],
) -> Vec<Lint> {
    if crate::template::validate(source).is_err() {
        return Vec::new();
    }

    let Ok(tokens) = tokenize(source, false, Default::default(), Default::default())
        .collect::<Result<Vec<_>, _>>()
    else {
        return Vec::new();
    };

    let mut lints = Vec::new();
    let mut in_block = false;
    let mut expr_start = 0;
    let mut for_stack: Vec<ForScope> = Vec::new();
    let mut set_vars: Vec<String> = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        let (ref token, span) = tokens[i];
        match token {
            Token::VariableStart => {
                in_block = true;
                expr_start = span.end_offset as usize;
            }
            Token::VariableEnd => {
                in_block = false;
            }
            Token::BlockStart => {
                in_block = true;
                expr_start = span.end_offset as usize;
            }
            Token::BlockEnd => {
                in_block = false;
                let tag_end = span.start_offset as usize;
                if expr_start <= tag_end && tag_end <= source.len() {
                    let tag_text = &source[expr_start..tag_end];
                    process_closed_tag(tag_text, inputs, types, &mut for_stack, &mut set_vars);
                }
            }
            Token::Pipe if in_block => {
                if i + 1 < tokens.len()
                    && let Token::Ident(filter_name) = &tokens[i + 1].0
                {
                    let filter_span = tokens[i + 1].1;
                    let range = filter_span.start_offset as usize..filter_span.end_offset as usize;

                    if is_known_filter(filter_name) {
                        let pipe_offset = span.start_offset as usize;
                        if expr_start <= pipe_offset && pipe_offset <= source.len() {
                            let left_text = &source[expr_start..pipe_offset];
                            let (_, for_stack_now, set_vars_now) =
                                scan_prefix(source, pipe_offset, inputs, types);
                            let ctx = InferContext {
                                inputs,
                                types,
                                for_stack: &for_stack_now,
                                set_vars: &set_vars_now,
                            };
                            let left_kind = infer_left(left_text, &ctx);
                            if left_kind != ValueKind::Any
                                && !filter_accepts(filter_name, &left_kind)
                            {
                                let kind_lbl = kind_label(&left_kind, types);
                                lints.push(Lint {
                                    range,
                                    message: UiMessage::new(
                                        "template-lint-filter-type",
                                        format!(
                                            "Filter '{filter_name}' cannot be applied to {kind_lbl}"
                                        ),
                                    )
                                    .arg("name", filter_name)
                                    .arg("type", &kind_lbl),
                                    severity: LintSeverity::Warning,
                                });
                            }
                        }
                    } else {
                        let mut candidates: Vec<&str> = BUILTIN_FILTERS
                            .iter()
                            .map(|(n, _)| *n)
                            .chain(std::iter::once("text"))
                            .collect();
                        candidates.sort_unstable();
                        candidates.dedup();
                        let suggestion = find_suggestion(filter_name, candidates);

                        if let Some(sugg) = suggestion {
                            lints.push(Lint {
                                range,
                                message: UiMessage::new(
                                    "template-lint-unknown-filter-suggest",
                                    format!(
                                        "Unknown filter '{filter_name}'. Did you mean '{sugg}'?"
                                    ),
                                )
                                .arg("name", filter_name)
                                .arg("suggestion", sugg),
                                severity: LintSeverity::Warning,
                            });
                        } else {
                            lints.push(Lint {
                                range,
                                message: UiMessage::new(
                                    "template-lint-unknown-filter",
                                    format!("Unknown filter '{filter_name}'"),
                                )
                                .arg("name", filter_name),
                                severity: LintSeverity::Warning,
                            });
                        }
                    }
                }
            }
            Token::Ident(word) if in_block && *word == "is" => {
                let prev_is_dot = i > 0 && matches!(tokens[i - 1].0, Token::Dot);
                if !prev_is_dot {
                    let test_info = if i + 1 < tokens.len()
                        && let Token::Ident(next_w) = &tokens[i + 1].0
                        && *next_w == "not"
                    {
                        if i + 2 < tokens.len()
                            && let Token::Ident(test_name) = &tokens[i + 2].0
                        {
                            Some((*test_name, tokens[i + 2].1))
                        } else {
                            None
                        }
                    } else if i + 1 < tokens.len()
                        && let Token::Ident(test_name) = &tokens[i + 1].0
                    {
                        Some((*test_name, tokens[i + 1].1))
                    } else {
                        None
                    };

                    if let Some((test_name, test_span)) = test_info
                        && !TESTS.contains(&test_name)
                    {
                        let range = test_span.start_offset as usize..test_span.end_offset as usize;
                        lints.push(Lint {
                            range,
                            message: UiMessage::new(
                                "template-lint-unknown-test",
                                format!("Unknown test '{test_name}'"),
                            )
                            .arg("name", test_name),
                            severity: LintSeverity::Warning,
                        });
                    }
                }
            }
            Token::Ident(root_name) if in_block => {
                let prev_is_dot = i > 0 && matches!(tokens[i - 1].0, Token::Dot);
                if !prev_is_dot
                    && i + 1 < tokens.len()
                    && matches!(tokens[i + 1].0, Token::Dot)
                    && (*root_name == "var" || *root_name == "sheet" || *root_name == "global")
                    && !variables.is_empty()
                {
                    if i + 2 < tokens.len()
                        && let Token::Ident(var_name) = &tokens[i + 2].0
                    {
                        let var_span = tokens[i + 2].1;
                        let scope_matches = |v: &&crate::Variable| match *root_name {
                            "sheet" => v.key.scope == crate::VariableScope::Sheet,
                            "global" => v.key.scope != crate::VariableScope::Sheet,
                            _ => true,
                        };
                        let found = variables
                            .iter()
                            .filter(scope_matches)
                            .find(|v| v.key.name == *var_name);
                        match found {
                            None => {
                                let range =
                                    var_span.start_offset as usize..var_span.end_offset as usize;
                                let candidates = variables
                                    .iter()
                                    .filter(scope_matches)
                                    .map(|v| v.key.name.as_str());
                                let suggestion = find_suggestion(var_name, candidates);
                                if let Some(sugg) = suggestion {
                                    lints.push(Lint {
                                        range,
                                        message: UiMessage::new(
                                            "template-lint-unknown-variable-suggest",
                                            format!("Unknown variable '{var_name}'. Did you mean '{sugg}'?"),
                                        )
                                        .arg("name", var_name)
                                        .arg("suggestion", sugg),
                                        severity: LintSeverity::Warning,
                                    });
                                } else {
                                    lints.push(Lint {
                                        range,
                                        message: UiMessage::new(
                                            "template-lint-unknown-variable",
                                            format!("Unknown variable '{var_name}'"),
                                        )
                                        .arg("name", var_name),
                                        severity: LintSeverity::Warning,
                                    });
                                }
                            }
                            Some(var) => {
                                check_struct_fields(
                                    &tokens,
                                    i + 2,
                                    var.data_type.clone(),
                                    types,
                                    &mut lints,
                                );
                            }
                        }
                    }
                } else if !prev_is_dot
                    && i + 1 < tokens.len()
                    && matches!(tokens[i + 1].0, Token::Dot)
                    && *root_name != "loop"
                    && *root_name != "text"
                    && *root_name != "var"
                    && *root_name != "sheet"
                    && *root_name != "global"
                    && !for_stack
                        .iter()
                        .any(|s| s.vars.iter().any(|v| v == root_name))
                    && !set_vars.iter().any(|v| v == root_name)
                    && let Some((_, root_type)) = inputs.iter().find(|(name, _)| name == root_name)
                {
                    check_struct_fields(&tokens, i, root_type.clone(), types, &mut lints);
                }
            }
            _ => {}
        }
        i += 1;
    }

    lints
}

fn check_struct_fields(
    tokens: &[(Token, minijinja::machinery::Span)],
    root_idx: usize,
    mut current_type: DataType,
    types: &TypeRegistry,
    lints: &mut Vec<Lint>,
) {
    let mut k = root_idx + 1;
    while k < tokens.len() && matches!(tokens[k].0, Token::Dot) {
        if k + 1 < tokens.len()
            && let Token::Ident(field_name) = &tokens[k + 1].0
        {
            let field_span = tokens[k + 1].1;
            let struct_info = match &current_type {
                DataType::Named(name) => types.get(name).and_then(|def| match &def.kind {
                    TypeKind::Struct { fields } => Some((def, fields)),
                    _ => None,
                }),
                _ => None,
            };

            match struct_info {
                None => break,
                Some((def, struct_fields)) => {
                    if let Some(f) = struct_fields.iter().find(|f| f.name == *field_name) {
                        current_type = f.data_type.clone();
                        k += 2;
                    } else {
                        let range =
                            field_span.start_offset as usize..field_span.end_offset as usize;
                        let type_label = if def.title.is_empty() {
                            def.name.clone()
                        } else {
                            def.title.clone()
                        };
                        let suggestion = find_suggestion(
                            field_name,
                            struct_fields.iter().map(|f| f.name.as_str()),
                        );
                        if let Some(sugg) = suggestion {
                            lints.push(Lint {
                                range,
                                message: UiMessage::new(
                                    "template-lint-unknown-field-suggest",
                                    format!(
                                        "Unknown field '{field_name}' on {type_label}. Did you mean '{sugg}'?"
                                    ),
                                )
                                .arg("field", field_name)
                                .arg("type", &type_label)
                                .arg("suggestion", sugg),
                                severity: LintSeverity::Warning,
                            });
                        } else {
                            lints.push(Lint {
                                range,
                                message: UiMessage::new(
                                    "template-lint-unknown-field",
                                    format!("Unknown field '{field_name}' on {type_label}"),
                                )
                                .arg("field", field_name)
                                .arg("type", &type_label),
                                severity: LintSeverity::Warning,
                            });
                        }
                        break;
                    }
                }
            }
        } else {
            break;
        }
    }
}
