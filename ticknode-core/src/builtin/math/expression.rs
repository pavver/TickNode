use super::numeric_type;
use crate::{Node, NodeContext};
use std::collections::HashMap;
use ticknode_shared::*;

#[cfg(test)]
mod tests;

const HELP: &str = r#"Evaluates an arbitrary mathematical formula set directly on the node ("Formula" row).

Syntax:
• Numbers: integers (42, 1_000_000), decimals (3.14), scientific notation (1e-3).
• Booleans: true, false.
• Constants: pi, e, tau, inf, nan.
• Arithmetic: + - * / % (modulo with sign of divisor), ^ or ** (power, right-associative).
• Comparisons: == != < <= > >=; Logic: && || ! (and or not); Ternary: c ? a : b.
• Functions: abs, sign, min, max, clamp(x, lo, hi), round(x[, digits]), floor, ceil, trunc, frac, sqrt, cbrt, pow, exp, ln, log(x[, base]), log10, log2, hypot, sin, cos, tan, asin, acos, atan, atan2, sinh, cosh, tanh, deg, rad, sum, avg/mean, median, lerp(a, b, t), map(x, in_lo, in_hi, out_lo, out_hi), if(c, a, b), mod(a, b), gcd, lcm, between(x, lo, hi), isnan, isfinite/finite.
• Compatibility: supports evalexpr names such as `math::sqrt(x)`.

Variables and pins:
Each variable in the formula automatically becomes an input pin. Undeclared variables stay optional with an inline default of 0. Renaming a single variable in place preserves its pin ID and wire.

Physical quantities:
Inputs and outputs support physical quantities (quantity.*). Operations between the same quantity keep that quantity. Adding or subtracting a plain number treats it as canonical units. Multiplying or dividing a quantity by a number keeps that quantity. Dividing two identical quantities produces a dimensionless Number. Output type defaults to 'auto' (inferred) or can be fixed to a specific quantity or Number.

Pure node: evaluated whenever its output is needed."#;

pub struct MathExpressionNode {
    pub id: NodeId,
    pub expression: String,
    pub value_type: String,
    pub compiled: Option<formula::CompiledFormula>,
    pub pins: Vec<(String, PinId, DataType)>,
    pub next_pin: u32,
    pub restoring: bool,
    pub restoration_started: bool,
    pub had_pin_ids: bool,
    pub reported: bool,
}

impl MathExpressionNode {
    pub fn new(id: NodeId) -> Self {
        let mut node = Self {
            id,
            expression: "a + b".to_string(),
            value_type: "auto".to_string(),
            compiled: None,
            pins: Vec::new(),
            next_pin: 1,
            restoring: true,
            restoration_started: false,
            had_pin_ids: false,
            reported: false,
        };
        node.compile();
        node
    }

    fn compile(&mut self) {
        let (compiled, ordered) = match formula::compile(&self.expression) {
            Ok(c) => {
                let vars = c.variables.clone();
                (Some(c), vars)
            }
            Err(_) => {
                let vars = formula::variables(&self.expression).unwrap_or_default();
                (None, vars)
            }
        };
        self.compiled = compiled;

        let old = std::mem::take(&mut self.pins);
        let mut rename: Option<(String, PinId, DataType)> = None;
        if !self.restoring {
            let disappeared: Vec<(usize, PinId, DataType)> = old
                .iter()
                .enumerate()
                .filter(|(_, (name, _, _))| !ordered.contains(name))
                .map(|(idx, (_, pin, ty))| (idx, *pin, ty.clone()))
                .collect();
            let appeared: Vec<(usize, &String)> = ordered
                .iter()
                .enumerate()
                .filter(|(_, name)| !old.iter().any(|(n, _, _)| n == *name))
                .collect();
            if disappeared.len() == 1 && appeared.len() == 1 && disappeared[0].0 == appeared[0].0 {
                rename = Some((
                    appeared[0].1.clone(),
                    disappeared[0].1,
                    disappeared[0].2.clone(),
                ));
            }
        }

        self.next_pin = old
            .iter()
            .map(|(_, pin, _)| pin.0 + 1)
            .max()
            .unwrap_or(1)
            .max(self.next_pin);

        self.pins = ordered
            .into_iter()
            .filter_map(|name| {
                if let Some((_, pin, ty)) = old.iter().find(|(n, _, _)| *n == name) {
                    Some((name, *pin, ty.clone()))
                } else if let Some((rename_name, pin, ty)) = &rename
                    && rename_name == &name
                {
                    Some((name, *pin, ty.clone()))
                } else {
                    if self.next_pin >= 0x8000_0000 {
                        return None;
                    }
                    let pin = PinId(self.next_pin);
                    self.next_pin += 1;
                    Some((name, pin, DataType::Number))
                }
            })
            .collect();
    }

    fn effective_output_type(&self) -> DataType {
        if self.value_type != "auto"
            && let Some(t) = parse_data_type(&self.value_type).filter(numeric_type)
        {
            return t;
        }
        if let Some(compiled) = &self.compiled {
            let map: HashMap<String, DataType> = self
                .pins
                .iter()
                .map(|(n, _, ty)| (n.clone(), ty.clone()))
                .collect();
            if let Ok(inferred) = compiled.infer_type(&map) {
                return inferred;
            }
        }
        DataType::Number
    }

    fn effective_output_type_in(&self, ctx: &NodeContext) -> DataType {
        if self.value_type != "auto"
            && let Some(t) = parse_data_type(&self.value_type).filter(numeric_type)
        {
            return t;
        }
        if let Some(compiled) = &self.compiled {
            let mut map: HashMap<String, DataType> = self
                .pins
                .iter()
                .map(|(n, _, ty)| (n.clone(), ty.clone()))
                .collect();
            for v in ctx.variables.for_sheet_and_global(ctx.sheet) {
                map.insert(format!("var.{}", v.key.name), v.data_type.clone());
                map.insert(format!("{}.{}", v.key.prefix(), v.key.name), v.data_type);
            }
            if let Ok(inferred) = compiled.infer_type(&map) {
                return inferred;
            }
        }
        DataType::Number
    }

    fn report_error(&mut self, ctx: &mut NodeContext, message: UiMessage) {
        if self.reported {
            ctx.host_state.report_error(self.id, message);
        } else {
            ctx.host_state.error(
                self.id,
                message.clone(),
                format!("[ERROR] {}", message.fallback),
            );
            self.reported = true;
        }
    }
}

impl Node for MathExpressionNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        let res_type = self.effective_output_type();
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: "Res".to_string(),
            pin_type: PinType::Data(res_type),
            direction: PinDirection::Output,
            required: false,
        }];
        for (name, pin_id, ty) in &self.pins {
            pins.push(PinDefinition {
                id: *pin_id,
                name: name.clone(),
                pin_type: PinType::Data(ty.clone()),
                direction: PinDirection::Input,
                required: false,
            });
        }
        NodeDefinition {
            adaptive: true,
            cost: 1,
            type_name: "math/expression".to_string(),
            title: "Formula".to_string(),
            category: "Math".to_string(),
            description: "Evaluates an arbitrary mathematical formula".to_string(),
            help: HELP.to_string(),
            properties: vec![
                PropertyDefinition::multiline("expression", "Formula")
                    .described("Mathematical formula; variables become input pins."),
                PropertyDefinition::text("value_type", "Output type")
                    .described("Output data type ('auto' inferred from inputs or an explicit number/quantity)."),
                PropertyDefinition::hidden("pin_ids"),
                PropertyDefinition::hidden("next_pin"),
            ],
            pins,
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let compiled = match &self.compiled {
            Some(c) => c,
            None => {
                let err_msg = match formula::validate(&self.expression) {
                    Err(e) => e.message,
                    Ok(_) => UiMessage::new("error-formula-syntax", "Invalid formula syntax"),
                };
                self.report_error(ctx, err_msg);
                return vec![];
            }
        };

        let lookup = |var_name: &str| -> Option<FormulaValue> {
            let (_, pin, _) = self.pins.iter().find(|(n, _, _)| n == var_name)?;
            match ctx.inputs.get(pin) {
                Some(Some(DataValue::Number(n))) => Some(FormulaValue::Number(*n)),
                Some(Some(DataValue::Bool(b))) => Some(FormulaValue::Bool(*b)),
                _ => Some(FormulaValue::Number(NumberValue::Int(0))),
            }
        };

        let var_lookup = |vref: &VariableRef| -> Result<FormulaValue, FormulaError> {
            let var = ctx.variable(&vref.name, vref.lookup).ok_or_else(|| {
                FormulaError::new(
                    0..self.expression.len(),
                    "error-variable-missing",
                    format!("Variable \"{}\" not found", vref.name),
                )
                .with_arg("name", &vref.name)
            })?;
            match &var.value {
                DataValue::Number(n) => Ok(FormulaValue::Number(*n)),
                DataValue::Bool(b) => Ok(FormulaValue::Bool(*b)),
                _ => Err(FormulaError::new(
                    0..self.expression.len(),
                    "error-formula-variable-type",
                    format!("Variable '{}' has non-numeric type", vref.name),
                )
                .with_arg("name", &vref.name)),
            }
        };

        match compiled.eval_with(&lookup, &var_lookup) {
            Ok(val) => {
                self.reported = false;
                let out_type = self.effective_output_type_in(ctx);
                let data_val = if out_type == DataType::Bool {
                    DataValue::Bool(val.is_truthy())
                } else {
                    DataValue::Number(val.to_number_value())
                };
                vec![NodeOutput::Data(PinId(0), data_val)]
            }
            Err(err) => {
                self.report_error(ctx, err.message);
                vec![]
            }
        }
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
        if name == "expression"
            && let DataValue::String(s) = value
        {
            formula::validate(s).map_err(|e| e.message)?;
        }
        if name == "value_type"
            && let DataValue::String(s) = value
        {
            if s != "auto" && parse_data_type(s).filter(numeric_type).is_none() {
                return Err(UiMessage::new(
                    "error-formula-type-mismatch",
                    format!("Invalid output type '{s}'"),
                ));
            }
        }
        Ok(())
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        let mut props = vec![
            (
                "expression".into(),
                DataValue::String(self.expression.clone()),
            ),
            (
                "pin_ids".into(),
                DataValue::List {
                    element_type: DataType::String,
                    elements: self
                        .pins
                        .iter()
                        .map(|(n, p, ty)| {
                            if *ty == DataType::Number {
                                format!("{}:{n}", p.0).into()
                            } else {
                                format!("{}:{n}:{}", p.0, data_type_name(ty)).into()
                            }
                        })
                        .collect(),
                },
            ),
            (
                "next_pin".into(),
                DataValue::Number(NumberValue::Int(self.next_pin.into())),
            ),
        ];
        if self.value_type != "auto" {
            props.push((
                "value_type".into(),
                DataValue::String(self.value_type.clone()),
            ));
        }
        props
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if self.restoring && !self.restoration_started {
            self.pins.clear();
            self.next_pin = 1;
            self.had_pin_ids = false;
            self.restoration_started = true;
        }

        match (name, value) {
            ("expression", DataValue::String(s)) => {
                self.expression = s;
            }
            ("value_type", DataValue::String(s)) => {
                self.value_type = s;
            }
            ("pin_ids", DataValue::List { elements, .. }) => {
                self.had_pin_ids = true;
                let mut pins = Vec::new();
                for val in elements {
                    let DataValue::String(s) = val else { continue };
                    let parts: Vec<&str> = s.split(':').collect();
                    if parts.len() < 2 {
                        continue;
                    }
                    let Ok(pin @ 1..=0x7fff_ffff) = parts[0].parse::<u32>() else {
                        continue;
                    };
                    let var_name = parts[1].to_string();
                    let ty = if parts.len() >= 3 {
                        parse_data_type(parts[2]).unwrap_or(DataType::Number)
                    } else {
                        DataType::Number
                    };
                    if !pins.iter().any(|(n, p, _): &(String, PinId, DataType)| {
                        *n == var_name || *p == PinId(pin)
                    }) {
                        pins.push((var_name, PinId(pin), ty));
                    }
                }
                self.pins = pins;
            }
            ("next_pin", DataValue::Number(NumberValue::Int(n)))
                if (1..=0x8000_0000).contains(&n) =>
            {
                self.next_pin = self.next_pin.max(n as u32);
            }
            _ => return,
        }

        if !self.restoring {
            self.compile();
        }
    }

    fn restored(&mut self) {
        if self.restoring {
            if !self.had_pin_ids {
                let vars = formula::variables(&self.expression).unwrap_or_default();
                let mut sorted = vars;
                sorted.sort();
                sorted.dedup();
                self.pins = sorted
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| (name, PinId(i as u32 + 1), DataType::Number))
                    .collect();
                self.next_pin = self.pins.iter().map(|(_, p, _)| p.0 + 1).max().unwrap_or(1);
            }
            self.compile();
            self.restoring = false;
        }
    }

    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin != PinId(0) && self.pins.iter().any(|(_, p, _)| *p == pin)
    }

    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        if pin == PinId(0) || !numeric_type(t) {
            return false;
        }
        let Some(entry) = self.pins.iter_mut().find(|(_, p, _)| *p == pin) else {
            return false;
        };
        if entry.2 == *t {
            return false;
        }
        entry.2 = t.clone();
        true
    }
}

/// Formula variables become input pins (1, 2, ...) in alphabetical order.
pub fn expression_variables(expression: &str) -> HashMap<String, PinId> {
    let mut names: Vec<String> = match formula::variables(expression) {
        Ok(vars) => vars,
        Err(_) => Vec::new(),
    };
    names.sort();
    names.dedup();
    names
        .into_iter()
        .enumerate()
        .map(|(i, n)| (n, PinId(i as u32 + 1)))
        .collect()
}
