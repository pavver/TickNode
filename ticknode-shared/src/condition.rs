//! Match node conditions in the style of Factorio's decider combinator. A node has explicit inputs
//! (`MatchInput`: pin, name, type) and cases (`MatchCase`): each case is a list of "input  operator  operand"
//! rows joined by "and" / "or" ("and" binds tighter: `a and b or c` = `(a and b) or c`).
//!
//! Operators depend on the left input's type (`ops_for`): comparisons for numbers and time,
//! "contains" / "starts with" for strings, "contains" / "length" for lists, etc. The right operand is another
//! input or a constant; a constant is coerced to the left value's type (`text::coerce_to`), so `"07:30"` is
//! compared against a time value as time. Unary operators ("is empty", "has value") have no right operand.
//!
//! Text representation (stored in node properties, human-readable in git):
//! input — `"1:a:number"`, case — `"<pin>: a > 10 and s contains \"on\" or l is_empty"`.

use std::cmp::Ordering;

use crate::text::{coerce_to, format_value};
use crate::{DataType, DataValue, NumberValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq, Ne, Lt, Le, Gt, Ge,
    Contains, NotContains, StartsWith, EndsWith,
    IsEmpty, NotEmpty, IsTrue, IsFalse,
    HasField, NoField,
    LenEq, LenGt, LenLt,
    HasValue, NoValue,
}

use CompareOp::*;

impl CompareOp {
    pub const ALL: [CompareOp; 21] = [
        Eq, Ne, Lt, Le, Gt, Ge, Contains, NotContains, StartsWith, EndsWith, IsEmpty, NotEmpty, IsTrue, IsFalse,
        HasField, NoField, LenEq, LenGt, LenLt, HasValue, NoValue,
    ];

    /// Storage (and input) representation.
    pub fn text(self) -> &'static str {
        match self {
            Eq => "=", Ne => "!=", Lt => "<", Le => "<=", Gt => ">", Ge => ">=",
            Contains => "contains", NotContains => "not_contains", StartsWith => "starts_with", EndsWith => "ends_with",
            IsEmpty => "is_empty", NotEmpty => "not_empty", IsTrue => "is_true", IsFalse => "is_false",
            HasField => "has_field", NoField => "no_field",
            LenEq => "len_eq", LenGt => "len_gt", LenLt => "len_lt",
            HasValue => "has_value", NoValue => "no_value",
        }
    }

    /// Display symbol in a condition on a node.
    pub fn symbol(self) -> &'static str {
        match self {
            Eq => "=", Ne => "≠", Lt => "<", Le => "≤", Gt => ">", Ge => "≥",
            Contains => "contains", NotContains => "not contains", StartsWith => "starts with", EndsWith => "ends with",
            IsEmpty => "is empty", NotEmpty => "is not empty", IsTrue => "= true", IsFalse => "= false",
            HasField => "has field", NoField => "no field",
            LenEq => "length =", LenGt => "length >", LenLt => "length <",
            HasValue => "has value", NoValue => "no value",
        }
    }

    /// Label in the operator selector — type-aware (for time, `<` means "earlier").
    pub fn label_for(self, t: &DataType) -> String {
        let time = matches!(t, DataType::Time | DataType::DateTime);
        match self {
            Lt if time => "< earlier".into(),
            Le if time => "≤ not later".into(),
            Gt if time => "> later".into(),
            Ge if time => "≥ not earlier".into(),
            Lt if *t == DataType::Duration => "< shorter".into(),
            Gt if *t == DataType::Duration => "> longer".into(),
            op => op.symbol().into(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "=" | "==" => Eq,
            "!=" | "≠" | "<>" => Ne,
            "<" => Lt,
            "<=" | "≤" => Le,
            ">" => Gt,
            ">=" | "≥" => Ge,
            _ => return CompareOp::ALL.into_iter().find(|op| op.text() == s),
        })
    }

    /// Whether the operator has no right operand.
    pub fn is_unary(self) -> bool {
        matches!(self, IsEmpty | NotEmpty | IsTrue | IsFalse | HasValue | NoValue)
    }

    /// Type of the right operand for a left operand of type `left` (`None` if the operator is unary).
    pub fn right_type(self, left: &DataType) -> Option<DataType> {
        if self.is_unary() {
            return None;
        }
        Some(match self {
            Contains | NotContains => match left {
                DataType::List(inner) => (**inner).clone(),
                DataType::Any => DataType::Any,
                _ => DataType::String,
            },
            StartsWith | EndsWith | HasField | NoField => DataType::String,
            LenEq | LenGt | LenLt => DataType::Number,
            _ => left.clone(),
        })
    }

    fn holds(self, ord: Ordering) -> bool {
        match self {
            Eq | LenEq => ord == Ordering::Equal,
            Ne => ord != Ordering::Equal,
            Lt | LenLt => ord == Ordering::Less,
            Le => ord != Ordering::Greater,
            Gt | LenGt => ord == Ordering::Greater,
            Ge => ord != Ordering::Less,
            _ => false,
        }
    }
}

/// Operators applicable to a left input of this type (in display order).
pub fn ops_for(t: &DataType) -> &'static [CompareOp] {
    match t {
        DataType::Number | DataType::Time | DataType::DateTime | DataType::Duration => &[Eq, Ne, Lt, Le, Gt, Ge, HasValue, NoValue],
        DataType::String => &[Eq, Ne, Contains, NotContains, StartsWith, EndsWith, IsEmpty, NotEmpty, LenEq, LenGt, LenLt, HasValue, NoValue],
        DataType::Bool => &[IsTrue, IsFalse, Eq, Ne, HasValue, NoValue],
        DataType::Color => &[Eq, Ne, HasValue, NoValue],
        DataType::List(_) => &[Contains, NotContains, IsEmpty, NotEmpty, LenEq, LenGt, LenLt, Eq, Ne, HasValue, NoValue],
        DataType::Object => &[HasField, NoField, IsEmpty, NotEmpty, Eq, Ne, HasValue, NoValue],
        // Custom type without a registry (enums; structs go through `ops_in`): equality and presence only
        DataType::Named(_) => &[Eq, Ne, HasValue, NoValue],
        DataType::Any => &[Eq, Ne, Lt, Le, Gt, Ge, Contains, NotContains, IsEmpty, NotEmpty, IsTrue, IsFalse, HasValue, NoValue],
    }
}

/// Like `ops_for`, but a struct from the registry behaves like an object (`has_field`, `is_empty`, etc.).
pub fn ops_in(types: &crate::TypeRegistry, t: &DataType) -> &'static [CompareOp] {
    match types.definition_of(t) {
        Some(def) if def.is_struct() => ops_for(&DataType::Object),
        _ => ops_for(t),
    }
}

/// Condition operand: a node input or a constant.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Input(String),
    Value(DataValue),
}

impl Operand {
    pub fn to_text(&self) -> String {
        match self {
            Self::Input(name) => name.clone(),
            Self::Value(DataValue::String(s)) => quote(s),
            Self::Value(DataValue::Number(NumberValue::Int(i))) => i.to_string(),
            Self::Value(DataValue::Number(NumberValue::Float(f))) => f.to_string(),
            Self::Value(DataValue::Bool(b)) => b.to_string(),
            // Time, duration, color, etc. are formatted as strings and coerced back to the input type on comparison
            Self::Value(other) => quote(&format_value(other)),
        }
    }

    fn resolve(&self, value_of: &impl Fn(&str) -> Option<DataValue>) -> Option<DataValue> {
        match self {
            Self::Input(name) => value_of(name),
            Self::Value(v) => Some(v.clone()),
        }
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join { And, Or }

impl Join {
    pub fn symbol(self) -> &'static str {
        match self { Self::And => "and", Self::Or => "or" }
    }
    fn text(self) -> &'static str {
        match self { Self::And => "and", Self::Or => "or" }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// How this row joins with the previous one (ignored on the first row).
    pub join: Join,
    pub left: Operand,
    pub op: CompareOp,
    /// `None` for unary operators.
    pub right: Option<Operand>,
}

impl Condition {
    /// An input without a value (unconnected and without an inline value on the node) does not satisfy
    /// the condition (except `NoValue`). An operator inapplicable to the value's type also evaluates to false.
    pub fn holds(&self, value_of: &impl Fn(&str) -> Option<DataValue>) -> bool {
        let a = self.left.resolve(value_of);
        match self.op {
            HasValue => return a.is_some(),
            NoValue => return a.is_none(),
            _ => {}
        }
        let Some(a) = a else { return false };
        if self.op.is_unary() {
            return unary(self.op, &a);
        }
        match self.right.as_ref().and_then(|r| r.resolve(value_of)) {
            Some(b) => binary(self.op, &a, b),
            None => false,
        }
    }

    fn to_text(&self, op: impl Fn(CompareOp) -> String, value: impl Fn(&Operand) -> String) -> String {
        match &self.right {
            Some(r) => format!("{} {} {}", value(&self.left), op(self.op), value(r)),
            None => format!("{} {}", value(&self.left), op(self.op)),
        }
    }
}

fn length(v: &DataValue) -> Option<usize> {
    match v {
        DataValue::String(s) => Some(s.chars().count()),
        DataValue::List { elements, .. } => Some(elements.len()),
        DataValue::Object { fields, .. } => Some(fields.len()),
        _ => None,
    }
}

fn unary(op: CompareOp, a: &DataValue) -> bool {
    match op {
        IsEmpty => length(a) == Some(0),
        NotEmpty => length(a).is_some_and(|n| n > 0),
        IsTrue => *a == DataValue::Bool(true),
        IsFalse => *a == DataValue::Bool(false),
        _ => false,
    }
}

/// Ordering for values of the same type (Int/Float numbers are compared together).
fn ordering(a: &DataValue, b: &DataValue) -> Option<Ordering> {
    match (a, b) {
        (DataValue::Number(x), DataValue::Number(y)) => x.to_f64().partial_cmp(&y.to_f64()),
        (DataValue::String(x), DataValue::String(y)) => Some(x.cmp(y)),
        (DataValue::Bool(x), DataValue::Bool(y)) => Some(x.cmp(y)),
        (DataValue::Time(x), DataValue::Time(y)) => Some(x.cmp(y)),
        (DataValue::DateTime(x), DataValue::DateTime(y)) => Some(x.cmp(y)),
        (DataValue::Duration(x), DataValue::Duration(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

/// Equality after coercing `b` to the type of `a` (`1` = `1.0`, `"07:30"` = time 07:30).
pub fn loosely_equal(a: &DataValue, b: &DataValue) -> bool {
    let b = coerce_to(b.clone(), &a.get_type());
    match ordering(a, &b) {
        Some(ord) => ord == Ordering::Equal,
        None => *a == b,
    }
}

fn binary(op: CompareOp, a: &DataValue, b: DataValue) -> bool {
    match op {
        Eq => loosely_equal(a, &b),
        Ne => !loosely_equal(a, &b),
        Lt | Le | Gt | Ge => ordering(a, &coerce_to(b, &a.get_type())).is_some_and(|ord| op.holds(ord)),
        Contains | NotContains => {
            let found = match a {
                DataValue::String(s) => Some(s.contains(&format_value(&b))),
                DataValue::List { elements, .. } => Some(elements.iter().any(|e| loosely_equal(e, &b))),
                DataValue::Object { fields, .. } => Some(fields.contains_key(&format_value(&b))),
                _ => None,
            };
            found.is_some_and(|f| f == (op == Contains))
        }
        StartsWith => matches!(a, DataValue::String(s) if s.starts_with(&format_value(&b))),
        EndsWith => matches!(a, DataValue::String(s) if s.ends_with(&format_value(&b))),
        HasField | NoField => match a {
            DataValue::Object { fields, .. } => fields.contains_key(&format_value(&b)) == (op == HasField),
            _ => false,
        },
        LenEq | LenGt | LenLt => match (length(a), coerce_to(b, &DataType::Number)) {
            (Some(n), DataValue::Number(m)) => (n as f64).partial_cmp(&m.to_f64()).is_some_and(|ord| op.holds(ord)),
            _ => false,
        },
        _ => false,
    }
}

/// A single output of a Match node: an exec pin and the conditions under which it fires (empty = always).
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub pin: u32,
    pub conditions: Vec<Condition>,
}

impl MatchCase {
    pub fn parse(text: &str) -> Result<MatchCase, String> {
        let (pin, rest) = text.split_once(':').ok_or("missing pin number (\"1: a > 0\")")?;
        let pin: u32 = pin.trim().parse().map_err(|_| format!("\"{}\" is not a pin number", pin.trim()))?;
        Ok(MatchCase { pin, conditions: parse_conditions(rest)? })
    }

    pub fn to_text(&self) -> String {
        let body = self.conditions.iter().enumerate().map(|(i, c)| {
            let cond = c.to_text(|op| op.text().to_string(), Operand::to_text);
            if i == 0 { cond } else { format!("{} {}", c.join.text(), cond) }
        }).collect::<Vec<_>>().join(" ");
        format!("{}: {}", self.pin, body).trim_end().to_string()
    }

    /// Human-readable display format using localized operator and join symbols.
    pub fn display(&self) -> String {
        if self.conditions.is_empty() {
            return "always".into();
        }
        self.conditions.iter().enumerate().map(|(i, c)| {
            let cond = c.to_text(|op| op.symbol().to_string(), Operand::to_text);
            if i == 0 { cond } else { format!("{} {}", c.join.symbol(), cond) }
        }).collect::<Vec<_>>().join(" ")
    }

    /// Disjunction ("or") of conjunction ("and") groups; empty conditions always match.
    pub fn matches(&self, value_of: impl Fn(&str) -> Option<DataValue>) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        let mut any = false;
        let mut group = true;
        for (i, c) in self.conditions.iter().enumerate() {
            if i > 0 && c.join == Join::Or {
                any |= group;
                group = true;
            }
            group = group && c.holds(&value_of);
        }
        any || group
    }

    /// Input names referenced by the conditions.
    pub fn inputs(&self) -> impl Iterator<Item = &str> {
        self.conditions.iter().flat_map(|c| std::iter::once(&c.left).chain(c.right.as_ref())).filter_map(|o| match o {
            Operand::Input(name) => Some(name.as_str()),
            Operand::Value(_) => None,
        })
    }

    /// Renames an input across all conditions.
    pub fn rename_input(&mut self, from: &str, to: &str) {
        for c in &mut self.conditions {
            for o in std::iter::once(&mut c.left).chain(c.right.as_mut()) {
                if matches!(o, Operand::Input(n) if n == from) {
                    *o = Operand::Input(to.to_string());
                }
            }
        }
    }
}

/// Explicit input of a Match node: an input data pin with a name referenced by conditions.
/// Type `Any` means "auto": the input accepts anything, and operators follow the actual connected type.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchInput {
    pub pin: u32,
    pub name: String,
    pub value_type: DataType,
}

/// Types selectable for a Match input (names match `input_type_name`).
pub const INPUT_TYPES: [&str; 10] = ["any", "number", "string", "bool", "time", "datetime", "duration", "color", "list", "object"];

pub fn input_type_name(t: &DataType) -> String {
    match t {
        DataType::List(inner) if **inner == DataType::Any => "list".into(),
        other => crate::data_type_name(other),
    }
}

pub fn parse_input_type(s: &str) -> Option<DataType> {
    match s.trim() {
        "list" | "list<any>" => Some(DataType::List(Box::new(DataType::Any))),
        other => crate::parse_data_type(other),
    }
}

impl MatchInput {
    pub fn parse(text: &str) -> Result<MatchInput, String> {
        let mut parts = text.splitn(3, ':');
        let (Some(pin), Some(name), Some(ty)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("\"{text}\": an input is written as \"pin:name:type\""));
        };
        let pin: u32 = pin.trim().parse().map_err(|_| format!("\"{}\" is not a pin number", pin.trim()))?;
        let name = name.trim().to_string();
        if !is_input_name(&name) {
            return Err(format!("\"{name}\" is not a valid input name"));
        }
        let value_type = parse_input_type(ty).ok_or_else(|| format!("\"{}\" is an unknown type", ty.trim()))?;
        Ok(MatchInput { pin, name, value_type })
    }

    pub fn to_text(&self) -> String {
        format!("{}:{}:{}", self.pin, self.name, input_type_name(&self.value_type))
    }
}

/// Inputs for legacy Match nodes (without an `inputs` property): sorted condition input names, type `Any`, pins `1..`.
pub fn match_inputs(cases: &[MatchCase]) -> Vec<String> {
    let mut names: Vec<String> = cases.iter().flat_map(|c| c.inputs().map(str::to_string)).collect();
    names.sort();
    names.dedup();
    names
}

pub fn is_input_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !matches!(s, "true" | "false" | "and" | "or" | "і" | "та" | "або")
        && CompareOp::parse(s).is_none()
}

fn parse_conditions(text: &str) -> Result<Vec<Condition>, String> {
    let mut tokens = tokenize(text)?.into_iter().peekable();
    let mut conditions = Vec::new();
    while tokens.peek().is_some() {
        let join = if conditions.is_empty() {
            Join::And
        } else {
            match tokens.next() {
                Some(Token::Join(j)) => j,
                other => return Err(format!("expected \"and\" / \"or\", not {}", describe(other.as_ref()))),
            }
        };
        let operand = |token: Option<Token>| {
            token.clone().and_then(Token::into_operand).ok_or_else(|| format!("expected an operand, not {}", describe(token.as_ref())))
        };
        let left = operand(tokens.next())?;
        let op = match tokens.next() {
            Some(Token::Op(op)) => op,
            Some(Token::Ident(word)) => CompareOp::parse(&word).ok_or_else(|| format!("\"{word}\" is an unknown operator"))?,
            other => return Err(format!("expected an operator, not {}", describe(other.as_ref()))),
        };
        let right = if op.is_unary() { None } else { Some(operand(tokens.next())?) };
        conditions.push(Condition { join, left, op, right });
    }
    Ok(conditions)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Value(DataValue),
    Op(CompareOp),
    Join(Join),
}

impl Token {
    fn into_operand(self) -> Option<Operand> {
        match self {
            Token::Ident(name) if is_input_name(&name) => Some(Operand::Input(name)),
            Token::Value(v) => Some(Operand::Value(v)),
            _ => None,
        }
    }
}

fn describe(token: Option<&Token>) -> String {
    match token {
        None => "end of text".into(),
        Some(Token::Ident(s)) => format!("«{s}»"),
        Some(Token::Value(v)) => format!("«{}»", Operand::Value(v.clone()).to_text()),
        Some(Token::Op(op)) => format!("«{}»", op.symbol()),
        Some(Token::Join(j)) => format!("«{}»", j.symbol()),
    }
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err("unclosed quote".into()),
                    Some('"') => break,
                    Some('\\') => {
                        s.extend(chars.get(i + 1));
                        i += 2;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            i += 1;
            tokens.push(Token::Value(DataValue::String(s)));
        } else if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            i += 1;
            while chars.get(i).is_some_and(|d| d.is_ascii_digit() || *d == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let f: f64 = s.parse().map_err(|_| format!("\"{s}\" is not a number"))?;
            tokens.push(Token::Value(DataValue::Number(NumberValue::Float(f).normalize())));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while chars.get(i).is_some_and(|d| d.is_alphanumeric() || *d == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(match word.as_str() {
                "true" => Token::Value(DataValue::Bool(true)),
                "false" => Token::Value(DataValue::Bool(false)),
                "and" | "і" | "та" => Token::Join(Join::And),
                "or" | "або" => Token::Join(Join::Or),
                _ => Token::Ident(word),
            });
        } else if c == '&' || c == '|' {
            if chars.get(i + 1) != Some(&c) {
                return Err(format!("\"{c}\" — probably \"{c}{c}\""));
            }
            i += 2;
            tokens.push(Token::Join(if c == '&' { Join::And } else { Join::Or }));
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let (op, len) = match CompareOp::parse(&two) {
                Some(op) if two.chars().count() == 2 => (op, 2),
                _ => (CompareOp::parse(&c.to_string()).ok_or_else(|| format!("unexpected character \"{c}\""))?, 1),
            };
            i += len;
            tokens.push(Token::Op(op));
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectField;
    use std::collections::BTreeMap;

    fn num(v: i64) -> Option<DataValue> {
        Some(DataValue::Number(NumberValue::Int(v)))
    }

    #[test]
    fn text_round_trip() {
        let case = MatchCase::parse("3: a > 10 and b != \"on \\\"x\\\"\" or c <= -2.5 or s contains \"x\" and l is_empty").unwrap();
        assert_eq!(case.pin, 3);
        assert_eq!(case.conditions.len(), 5);
        assert_eq!(case.conditions[2].join, Join::Or);
        assert_eq!(case.conditions[4].right, None);
        assert_eq!(MatchCase::parse(&case.to_text()).unwrap(), case);
        assert_eq!(case.display(), "a > 10 and b ≠ \"on \\\"x\\\"\" or c ≤ -2.5 or s contains \"x\" and l is empty");
        assert_eq!(MatchCase::parse("7:").unwrap().display(), "always");
        assert_eq!(MatchCase::parse("1: t ≥ 5 && v == true").unwrap().to_text(), "1: t >= 5 and v = true");
        assert_eq!(MatchCase::parse("1: a = b").unwrap().inputs().collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn parse_errors() {
        assert!(MatchCase::parse("a > 1").is_err());
        assert!(MatchCase::parse("1: a >").is_err());
        assert!(MatchCase::parse("1: a > 1 b < 2").is_err());
        assert!(MatchCase::parse("1: a ! 1").is_err());
        assert!(MatchCase::parse("1: a frobnicate 1").is_err());
        assert!(MatchCase::parse("1: contains > 1").is_err(), "an operator cannot be an input name");
        assert!(MatchInput::parse("1:a").is_err());
        assert!(MatchInput::parse("1:2a:number").is_err());
        assert!(MatchInput::parse("1:a:weird").is_err());
        let input = MatchInput::parse("4:temp:list").unwrap();
        assert_eq!(input.value_type, DataType::List(Box::new(DataType::Any)));
        assert_eq!(input.to_text(), "4:temp:list");
    }

    #[test]
    fn and_binds_tighter_than_or() {
        let case = MatchCase::parse("1: a > 1 and b > 1 or c > 1").unwrap();
        let eval = |a, b, c| case.matches(|n| match n { "a" => num(a), "b" => num(b), "c" => num(c), _ => None });
        assert!(eval(2, 2, 0));
        assert!(!eval(2, 0, 0));
        assert!(eval(0, 0, 2));
    }

    #[test]
    fn typed_comparisons() {
        let list = DataValue::List { element_type: DataType::Number, elements: vec![num(1).unwrap(), num(5).unwrap()] };
        let mut fields = BTreeMap::new();
        fields.insert("temp".to_string(), ObjectField { name: "temp".into(), description: String::new(), value: num(20).unwrap() });
        let object = DataValue::Object { fields, type_name: None };
        let holds = |text: &str| MatchCase::parse(&format!("1: {text}")).unwrap().matches(|n| match n {
            "s" => Some(DataValue::String("living room".into())),
            "n" => num(5),
            "m" => Some(DataValue::Number(NumberValue::Float(5.0))),
            "t" => Some(DataValue::Time(crate::text::parse_time("07:30").unwrap())),
            "d" => Some(DataValue::Duration(std::time::Duration::from_secs(90))),
            "l" => Some(list.clone()),
            "o" => Some(object.clone()),
            "f" => Some(DataValue::Bool(false)),
            _ => None,
        });
        // Numbers and two inputs
        assert!(holds("n >= 5") && holds("n = m") && holds("n != \"6\"") && !holds("n < \"5\""));
        // Strings
        assert!(holds("s contains \"room\"") && holds("s starts_with \"living\"") && holds("s ends_with \"room\""));
        assert!(holds("s not_contains \"kitchen\"") && holds("s not_empty") && holds("s len_gt 5"));
        // Time, duration — string constant is coerced to the input type
        assert!(holds("t < \"08:00\"") && holds("t = \"07:30\"") && !holds("t > \"07:30:01\""));
        assert!(holds("d > \"1m\"") && holds("d = 90") && holds("d < \"2m\""));
        // Lists, objects, booleans
        assert!(holds("l contains 5") && holds("l not_contains 7") && holds("l len_eq 2") && !holds("l is_empty"));
        assert!(holds("o has_field \"temp\"") && holds("o no_field \"hum\""));
        assert!(holds("f is_false") && !holds("f is_true"));
        // Missing value
        assert!(!holds("missing = 0") && !holds("missing != 0") && holds("missing no_value") && holds("n has_value"));
        // Operator inapplicable to type — evaluates to false
        assert!(!holds("n starts_with \"5\"") && !holds("f contains 1"));
    }

    #[test]
    fn ops_follow_type() {
        assert!(ops_for(&DataType::String).contains(&Contains));
        assert!(!ops_for(&DataType::Number).contains(&Contains));
        assert_eq!(Contains.right_type(&DataType::List(Box::new(DataType::Number))), Some(DataType::Number));
        assert_eq!(LenGt.right_type(&DataType::String), Some(DataType::Number));
        assert_eq!(IsEmpty.right_type(&DataType::String), None);
        assert_eq!(Lt.label_for(&DataType::Time), "< earlier");
    }
}
