//! Formula engine: tokenizer, Pratt parser, evaluator, type inference and catalog.

mod ast;
mod catalog;
mod error;
mod eval;
mod inference;
mod lexer;
mod parser;
#[cfg(test)]
mod tests;

pub use ast::{BinaryOp, Expr, ExprKind, UnaryOp};
pub use catalog::{CATALOG, FunctionDoc, catalog, find_function};
pub use error::FormulaError;
pub use eval::{FormulaValue, eval_expr};
pub use inference::infer_type;

use crate::DataType;
use std::collections::HashMap;

/// Pre-compiled formula AST with pre-extracted ordered variables.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledFormula {
    pub ast: Expr,
    pub variables: Vec<String>,
}

impl CompiledFormula {
    pub fn new(source: &str) -> Result<Self, FormulaError> {
        compile(source)
    }

    pub fn eval<F>(&self, lookup: &F) -> Result<FormulaValue, FormulaError>
    where
        F: Fn(&str) -> Option<FormulaValue>,
    {
        eval_expr(&self.ast, lookup)
    }

    pub fn infer_type(
        &self,
        input_types: &HashMap<String, DataType>,
    ) -> Result<DataType, FormulaError> {
        infer_type(&self.ast, input_types)
    }
}

/// Parses and validates formula syntax.
pub fn validate(source: &str) -> Result<(), FormulaError> {
    compile(source).map(|_| ())
}

/// Compiles a formula string into a [`CompiledFormula`].
pub fn compile(source: &str) -> Result<CompiledFormula, FormulaError> {
    let mut lexer = lexer::Lexer::new(source)?;
    let tokens = lexer.tokenize()?;
    let mut parser = parser::Parser::new(tokens);
    let ast = parser.parse()?;
    let mut vars = Vec::new();
    collect_variables(&ast, &mut vars);
    Ok(CompiledFormula {
        ast,
        variables: vars,
    })
}

/// Extracts variables from a formula in order of first appearance, deduplicated.
pub fn variables(source: &str) -> Result<Vec<String>, FormulaError> {
    let compiled = compile(source)?;
    Ok(compiled.variables)
}

fn collect_variables(expr: &Expr, vars: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Variable(name) => {
            if !vars.contains(name) {
                vars.push(name.clone());
            }
        }
        ExprKind::Unary { expr: inner, .. } => collect_variables(inner, vars),
        ExprKind::Binary { left, right, .. } => {
            collect_variables(left, vars);
            collect_variables(right, vars);
        }
        ExprKind::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_variables(cond, vars);
            collect_variables(then_expr, vars);
            collect_variables(else_expr, vars);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                collect_variables(arg, vars);
            }
        }
        ExprKind::Number(_) | ExprKind::Bool(_) => {}
    }
}
