//! The `unit` template filter: To text presentation of numbers and physical quantities.

use super::values;
use crate::{
    DataType, DataValue, NumberValue, Rounding, TypeKind, TypeRegistry, quantity,
    to_text::ToTextOptions,
};
use minijinja::{
    machinery::{Token, tokenize},
    value::{Kwargs, Rest, Value},
};
use std::collections::{HashMap, HashSet};

/// Scans the template source for expressions of the form `Ident(root) (Dot Ident)* Pipe Ident("unit")`
/// where `root` is not a local (`set`/`for`/`loop`/`text`).
///
/// Rewrites calls to pass the static dotted path via `__path='...'` keyword argument.
/// Returns the rewritten source and a list of `(original_offset, inserted_len)` records
/// for mapping error positions back to the original source.
pub fn rewrite_unit_paths(source: &str) -> (String, Vec<(usize, usize)>) {
    let Ok(tokens) = tokenize(source, false, Default::default(), Default::default())
        .collect::<Result<Vec<_>, _>>()
    else {
        return (source.to_string(), Vec::new());
    };

    let mut locals = HashSet::new();
    locals.insert("loop".to_string());
    locals.insert("text".to_string());

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i].0 {
            Token::Ident(word) if *word == "set" => {
                let mut j = i + 1;
                while j < tokens.len() && !matches!(&tokens[j].0, Token::Assign | Token::BlockEnd) {
                    if let Token::Ident(name) = &tokens[j].0 {
                        locals.insert(name.to_string());
                    }
                    j += 1;
                }
            }
            Token::Ident(word) if *word == "for" => {
                let mut j = i + 1;
                while j < tokens.len() {
                    match &tokens[j].0 {
                        Token::Ident(name) if *name == "in" => break,
                        Token::Ident(name) => {
                            locals.insert(name.to_string());
                        }
                        _ => {}
                    }
                    j += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut insertions: Vec<(usize, String)> = Vec::new();

    let mut k = 0;
    while k < tokens.len() {
        if let Token::Ident(name) = &tokens[k].0
            && *name == "unit"
            && k >= 2
            && matches!(tokens[k - 1].0, Token::Pipe)
        {
            let mut p = k - 1;
            let mut segments = Vec::new();
            if p > 0 {
                p -= 1;
                let mut expecting_ident = true;
                loop {
                    if expecting_ident {
                        if let Token::Ident(name) = &tokens[p].0 {
                            segments.push(name.to_string());
                            expecting_ident = false;
                            if p == 0 {
                                break;
                            }
                            p -= 1;
                        } else {
                            break;
                        }
                    } else if matches!(tokens[p].0, Token::Dot) {
                        expecting_ident = true;
                        if p == 0 {
                            break;
                        }
                        p -= 1;
                    } else {
                        break;
                    }
                }
            }

            let valid_root = !segments.is_empty()
                && (p == 0 && !segments.is_empty() || !matches!(tokens[p].0, Token::Pipe));

            if valid_root {
                segments.reverse();
                let root = &segments[0];
                if !locals.contains(root) {
                    let path = segments.join(".");
                    let unit_span = tokens[k].1;
                    if k + 1 < tokens.len() && matches!(tokens[k + 1].0, Token::ParenOpen) {
                        let open_span = tokens[k + 1].1;
                        let mut depth = 1;
                        let mut close_span = None;
                        for item in tokens.iter().skip(k + 2) {
                            match &item.0 {
                                Token::ParenOpen => depth += 1,
                                Token::ParenClose => {
                                    depth -= 1;
                                    if depth == 0 {
                                        close_span = Some(item.1);
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(close_span) = close_span {
                            let open_end = open_span.end_offset as usize;
                            let close_start = close_span.start_offset as usize;
                            let inner = &source[open_end..close_start];
                            let trimmed = inner.trim();
                            if trimmed.is_empty() || trimmed.ends_with(',') {
                                insertions.push((close_start, format!("__path='{path}'")));
                            } else {
                                insertions.push((close_start, format!(", __path='{path}'")));
                            }
                        }
                    } else {
                        let unit_end = unit_span.end_offset as usize;
                        insertions.push((unit_end, format!("(__path='{path}')")));
                    }
                }
            }
        }
        k += 1;
    }

    insertions.sort_by_key(|&(offset, _)| offset);

    let mut rewritten = String::with_capacity(source.len() + 64);
    let mut last_end = 0;
    let mut records = Vec::new();

    for (offset, insert_str) in insertions {
        rewritten.push_str(&source[last_end..offset]);
        let insert_len = insert_str.len();
        rewritten.push_str(&insert_str);
        last_end = offset;
        records.push((offset, insert_len));
    }
    rewritten.push_str(&source[last_end..]);

    (rewritten, records)
}

/// Maps a byte offset in the rewritten source back to the original source.
pub fn map_offset(offset: usize, records: &[(usize, usize)]) -> usize {
    let mut current_shift = 0;
    for &(orig_offset, inserted_len) in records {
        let rewritten_start = orig_offset + current_shift;
        let rewritten_end = rewritten_start + inserted_len;
        if offset < rewritten_start {
            break;
        }
        if offset <= rewritten_end {
            return orig_offset;
        }
        current_shift += inserted_len;
    }
    offset.saturating_sub(current_shift)
}

/// Builds path-to-type and path-to-value lookup maps from inputs and struct definitions.
pub fn build_path_maps(
    inputs: &[(String, Option<DataValue>, DataType)],
    types: &TypeRegistry,
) -> (HashMap<String, DataType>, HashMap<String, DataValue>) {
    let mut path_types = HashMap::new();
    let mut path_values = HashMap::new();
    for (name, val, ty) in inputs {
        populate_paths(
            name,
            val.as_ref(),
            ty,
            types,
            &mut path_types,
            &mut path_values,
            0,
        );
    }
    (path_types, path_values)
}

fn populate_paths(
    prefix: &str,
    val: Option<&DataValue>,
    ty: &DataType,
    types: &TypeRegistry,
    path_types: &mut HashMap<String, DataType>,
    path_values: &mut HashMap<String, DataValue>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    path_types.insert(prefix.to_string(), ty.clone());
    if let Some(v) = val {
        path_values.insert(prefix.to_string(), v.clone());
    }

    if let DataType::Named(name) = ty
        && let Some(def) = types.get(name)
        && let TypeKind::Struct { fields } = &def.kind
    {
        for field in fields {
            let child_prefix = format!("{prefix}.{}", field.name);
            let child_val = match val {
                Some(DataValue::Object {
                    fields: obj_fields, ..
                }) => obj_fields.get(&field.name).map(|f| &f.value),
                _ => None,
            };
            populate_paths(
                &child_prefix,
                child_val,
                &field.data_type,
                types,
                path_types,
                path_values,
                depth + 1,
            );
        }
    }
}

/// Implementation of the `| unit` filter.
pub fn unit_filter(
    to_text_fn: &(dyn Fn(&DataValue, &DataType, &ToTextOptions) -> String + Send + Sync),
    path_types: &HashMap<String, DataType>,
    path_values: &HashMap<String, DataValue>,
    value: Value,
    args: Rest<Value>,
    kwargs: Kwargs,
) -> Result<Value, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(Value::UNDEFINED);
    }

    let path: Option<String> = kwargs.get("__path")?;
    let digits: Option<u8> = kwargs.get("digits")?;
    let decimals: Option<u8> = kwargs.get("decimals")?;
    let units: Option<bool> = kwargs.get("units")?;
    let kw_symbol: Option<String> = kwargs.get("symbol")?;
    kwargs.assert_all_used()?;

    let symbol: Option<String> = args
        .0
        .first()
        .and_then(|v| v.as_str().map(str::to_string))
        .or(kw_symbol);

    let mut options = ToTextOptions::default();
    if let Some(show) = units {
        options.show_units = show;
    }
    if let Some(d) = decimals {
        options.rounding = Some(Rounding::Decimals);
        options.digits = Some(d.clamp(0, 15));
    } else if let Some(n) = digits {
        options.rounding = Some(Rounding::Significant);
        options.digits = Some(n.clamp(1, 15));
    }

    let declared_from_path = path.as_deref().and_then(|p| path_types.get(p));

    let effective_type = if let Some(ref sym) = symbol {
        let (sym_quantity_name, unit_def) = quantity::unit_by_symbol(sym).ok_or_else(|| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("unknown unit symbol: {sym}"),
            )
        })?;
        options.unit = Some(unit_def.id.clone());

        if let Some(DataType::Named(decl_name)) = declared_from_path
            && quantity::kind(decl_name).is_some()
            && decl_name != sym_quantity_name
        {
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("unit {sym} does not belong to {decl_name}"),
            ));
        }
        DataType::Named(sym_quantity_name.to_string())
    } else if let Some(decl) = declared_from_path {
        decl.clone()
    } else {
        DataType::Number
    };

    let data_val = if let Some(p) = path.as_deref()
        && let Some(orig_val) = path_values.get(p)
        && !matches!(effective_type, DataType::Number)
        && !matches!(&effective_type, DataType::Named(n) if quantity::kind(n).is_some())
    {
        orig_val.clone()
    } else if let Some(dv) = values::data_value(&value) {
        dv
    } else if let Some(p) = path.as_deref()
        && let Some(orig_val) = path_values.get(p)
    {
        orig_val.clone()
    } else if value.is_integer() {
        DataValue::Number(NumberValue::Int(value.as_i64().unwrap_or(0)))
    } else if let Ok(f) = f64::try_from(value.clone()) {
        DataValue::Number(NumberValue::Float(f))
    } else {
        DataValue::String(value.to_string())
    };

    let text = to_text_fn(&data_val, &effective_type, &options);
    Ok(Value::from(text))
}

/// Registers the localized `unit` filter on the MiniJinja environment.
pub fn register(
    env: &mut minijinja::Environment<'static>,
    env_ctx: &super::RenderEnv,
    inputs: &[(String, Option<DataValue>, DataType)],
    types: &TypeRegistry,
) {
    let (mut path_types, mut path_values) = build_path_maps(inputs, types);
    for v in &env_ctx.variables {
        match v.key.scope {
            crate::VariableScope::Sheet => {
                populate_paths(
                    &format!("sheet.{}", v.key.name),
                    Some(&v.value),
                    &v.data_type,
                    types,
                    &mut path_types,
                    &mut path_values,
                    0,
                );
                populate_paths(
                    &format!("var.{}", v.key.name),
                    Some(&v.value),
                    &v.data_type,
                    types,
                    &mut path_types,
                    &mut path_values,
                    0,
                );
            }
            crate::VariableScope::Global | crate::VariableScope::Persistent => {
                populate_paths(
                    &format!("global.{}", v.key.name),
                    Some(&v.value),
                    &v.data_type,
                    types,
                    &mut path_types,
                    &mut path_values,
                    0,
                );
                let var_path = format!("var.{}", v.key.name);
                if !path_types.contains_key(&var_path) {
                    populate_paths(
                        &var_path,
                        Some(&v.value),
                        &v.data_type,
                        types,
                        &mut path_types,
                        &mut path_values,
                        0,
                    );
                }
            }
        }
    }
    for (path, value, ty) in &env_ctx.plugin_values {
        path_types.insert(path.clone(), ty.clone());
        if let Some(value) = value {
            path_values.insert(path.clone(), value.clone());
        }
    }
    let to_text_fn = env_ctx.to_text.clone();
    env.add_filter(
        "unit",
        move |value: Value, args: Rest<Value>, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
            unit_filter(&*to_text_fn, &path_types, &path_values, value, args, kwargs)
        },
    );
}

#[cfg(test)]
mod tests;
