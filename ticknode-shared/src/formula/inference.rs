use super::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use super::error::FormulaError;
use crate::DataType;
use std::collections::HashMap;

/// Infers the output DataType (Number, Bool, or a core quantity type) from the formula AST
/// and known input variable types.
pub fn infer_type(
    expr: &Expr,
    input_types: &HashMap<String, DataType>,
) -> Result<DataType, FormulaError> {
    match &expr.kind {
        ExprKind::Number(_) => Ok(DataType::Number),
        ExprKind::Bool(_) => Ok(DataType::Bool),
        ExprKind::Variable(name) => Ok(input_types.get(name).cloned().unwrap_or(DataType::Number)),
        ExprKind::VariableRef(vref) => Ok(input_types
            .get(&vref.to_str())
            .or_else(|| input_types.get(&vref.name))
            .cloned()
            .unwrap_or(DataType::Number)),
        ExprKind::Unary { op, expr: inner } => {
            let t = infer_type(inner, input_types)?;
            match op {
                UnaryOp::Not => Ok(DataType::Bool),
                UnaryOp::Pos | UnaryOp::Neg => Ok(t),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let tl = infer_type(left, input_types)?;
            let tr = infer_type(right, input_types)?;
            match op {
                BinaryOp::And | BinaryOp::Or => Ok(DataType::Bool),
                BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge => {
                    if let (DataType::Named(q1), DataType::Named(q2)) = (&tl, &tr)
                        && q1 != q2
                    {
                        return Err(FormulaError::incompatible_quantities(
                            expr.span.clone(),
                            q1,
                            q2,
                        ));
                    }
                    Ok(DataType::Bool)
                }
                BinaryOp::Add | BinaryOp::Sub => match (&tl, &tr) {
                    (DataType::Named(q1), DataType::Named(q2)) => {
                        if q1 != q2 {
                            return Err(FormulaError::incompatible_quantities(
                                expr.span.clone(),
                                q1,
                                q2,
                            ));
                        }
                        Ok(DataType::Named(q1.clone()))
                    }
                    (DataType::Named(q), _) | (_, DataType::Named(q)) => {
                        Ok(DataType::Named(q.clone()))
                    }
                    _ => Ok(DataType::Number),
                },
                BinaryOp::Mul => match (&tl, &tr) {
                    (DataType::Named(q), DataType::Number)
                    | (DataType::Number, DataType::Named(q)) => Ok(DataType::Named(q.clone())),
                    _ => Ok(DataType::Number),
                },
                BinaryOp::Div => match (&tl, &tr) {
                    (DataType::Named(q1), DataType::Named(q2)) if q1 == q2 => Ok(DataType::Number),
                    (DataType::Named(q), DataType::Number) => Ok(DataType::Named(q.clone())),
                    _ => Ok(DataType::Number),
                },
                BinaryOp::Rem => match (&tl, &tr) {
                    (DataType::Named(q), _) => Ok(DataType::Named(q.clone())),
                    _ => Ok(DataType::Number),
                },
                BinaryOp::Pow => Ok(DataType::Number),
            }
        }
        ExprKind::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            let _ = infer_type(cond, input_types)?;
            let t1 = infer_type(then_expr, input_types)?;
            let t2 = infer_type(else_expr, input_types)?;
            reconcile_branches(&t1, &t2, expr.span.clone())
        }
        ExprKind::Call { name, args } => {
            let arg_types: Vec<DataType> = args
                .iter()
                .map(|a| infer_type(a, input_types))
                .collect::<Result<_, _>>()?;

            match name.as_str() {
                "if" => reconcile_branches(&arg_types[1], &arg_types[2], expr.span.clone()),
                "between" | "isnan" | "isfinite" => Ok(DataType::Bool),
                "min" | "max" | "clamp" | "avg" | "mean" | "sum" | "median" | "lerp" | "abs"
                | "round" | "floor" | "ceil" | "trunc" | "frac" => {
                    let mut found_quantity: Option<String> = None;
                    for (i, ty) in arg_types.iter().enumerate() {
                        if let DataType::Named(q) = ty {
                            if let Some(existing) = &found_quantity {
                                if existing != q {
                                    return Err(FormulaError::incompatible_quantities(
                                        args[i].span.clone(),
                                        existing,
                                        q,
                                    ));
                                }
                            } else {
                                found_quantity = Some(q.clone());
                            }
                        }
                    }
                    if let Some(q) = found_quantity {
                        Ok(DataType::Named(q))
                    } else {
                        Ok(DataType::Number)
                    }
                }
                "map" => match (&arg_types[3], &arg_types[4]) {
                    (DataType::Named(q1), DataType::Named(q2)) if q1 == q2 => {
                        Ok(DataType::Named(q1.clone()))
                    }
                    (DataType::Named(q), _) | (_, DataType::Named(q)) => {
                        Ok(DataType::Named(q.clone()))
                    }
                    _ => Ok(DataType::Number),
                },
                _ => Ok(DataType::Number),
            }
        }
    }
}

fn reconcile_branches(
    t1: &DataType,
    t2: &DataType,
    span: std::ops::Range<usize>,
) -> Result<DataType, FormulaError> {
    match (t1, t2) {
        (DataType::Named(q1), DataType::Named(q2)) => {
            if q1 != q2 {
                return Err(FormulaError::incompatible_quantities(span, q1, q2));
            }
            Ok(DataType::Named(q1.clone()))
        }
        (DataType::Named(q), _) | (_, DataType::Named(q)) => Ok(DataType::Named(q.clone())),
        (DataType::Bool, DataType::Bool) => Ok(DataType::Bool),
        _ => Ok(DataType::Number),
    }
}
