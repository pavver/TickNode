//! CLDR plural filter for MiniJinja templates.

use crate::DataValue;
use crate::plural::{PluralCategory, category, integer_categories};
use crate::quantity::QuantityLabels;
use crate::template::{RuntimeText, values};
use minijinja::value::{Kwargs, Rest, Value, ValueKind};
use minijinja::{Environment, ErrorKind};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
mod tests;

fn parse_number(value: &Value) -> Result<Option<(f64, Value)>, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(None);
    }
    if value.kind() == ValueKind::Number {
        if let Some(i) = value.as_i64() {
            return Ok(Some((i as f64, value.clone())));
        }
        if let Ok(f) = f64::try_from(value.clone())
            && f.is_finite()
        {
            return Ok(Some((f, value.clone())));
        }
    }
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if let Ok(f) = trimmed.parse::<f64>()
            && f.is_finite()
        {
            let val = if !trimmed.contains('.') && !trimmed.contains('e') && !trimmed.contains('E')
            {
                if let Ok(i) = trimmed.parse::<i64>() {
                    Value::from(i)
                } else {
                    Value::from(f)
                }
            } else {
                Value::from(f)
            };
            return Ok(Some((f, val)));
        }
        if trimmed.contains(',') {
            let replaced = trimmed.replace(',', ".");
            if let Ok(f) = replaced.parse::<f64>()
                && f.is_finite()
            {
                return Ok(Some((f, Value::from(f))));
            }
        }
    }
    Err(minijinja::Error::new(
        ErrorKind::InvalidOperation,
        "plural expects a number",
    ))
}

fn format_number_value(
    value: &Value,
    labels: &dyn QuantityLabels,
    runtime_text: Option<&(dyn Fn(&DataValue) -> String + Send + Sync)>,
    int_decimal_suffix: &str,
) -> String {
    if let Some(rt) = runtime_text {
        let formatted = values::format_runtime_value(value, rt, int_decimal_suffix);
        let sep = labels.decimal_separator();
        if sep != "." && formatted.contains('.') {
            return formatted.replace('.', &sep);
        }
        return formatted;
    }
    if value.is_integer() {
        return value.to_string();
    }
    if let Ok(f) = f64::try_from(value.clone()) {
        let mut s = format!("{f}");
        if s.contains('.') {
            s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
        return s.replace('.', &labels.decimal_separator());
    }
    value.to_string()
}

pub(super) fn plural_filter(
    labels: &dyn QuantityLabels,
    runtime_text: Option<&(dyn Fn(&DataValue) -> String + Send + Sync)>,
    int_decimal_suffix: &str,
    value: Value,
    args: Rest<Value>,
    kwargs: Kwargs,
) -> Result<Value, minijinja::Error> {
    let Some((n, num_val)) = parse_number(&value)? else {
        return Ok(Value::UNDEFINED);
    };

    let positional: Vec<String> = args.0.into_iter().map(|v| v.to_string()).collect();

    let mut keyword_forms = Vec::new();
    for name in kwargs.args() {
        let cat = match name {
            "zero" => Some(PluralCategory::Zero),
            "one" => Some(PluralCategory::One),
            "two" => Some(PluralCategory::Two),
            "few" => Some(PluralCategory::Few),
            "many" => Some(PluralCategory::Many),
            "other" => Some(PluralCategory::Other),
            _ => None,
        };
        if let Some(cat) = cat
            && let Ok(Some(val)) = kwargs.get::<Option<Value>>(name)
        {
            keyword_forms.push((cat, val.to_string()));
        }
    }

    let locale = labels.locale();
    let int_cats = integer_categories(&locale);

    let mut forms = BTreeMap::new();
    for (&cat, form) in int_cats.iter().zip(&positional) {
        forms.insert(cat, form.clone());
    }
    for &(cat, ref form) in &keyword_forms {
        forms.insert(cat, form.clone());
    }

    let cat = category(&locale, n);
    let chosen: Option<&str> = if let Some(form) = forms.get(&cat) {
        Some(form.as_str())
    } else if let Some(form) = forms.get(&PluralCategory::Other) {
        Some(form.as_str())
    } else if let Some(last_pos) = positional.last() {
        Some(last_pos.as_str())
    } else if let Some((_, last_kw)) = keyword_forms.last() {
        Some(last_kw.as_str())
    } else {
        None
    };

    let Some(chosen) = chosen else {
        return Ok(Value::from(""));
    };

    if chosen.contains('#') {
        let number_str = format_number_value(&num_val, labels, runtime_text, int_decimal_suffix);
        Ok(Value::from(chosen.replace('#', &number_str)))
    } else {
        Ok(Value::from(chosen))
    }
}

pub(super) fn register(
    env: &mut Environment<'static>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
    runtime_text: Option<RuntimeText>,
    int_decimal_suffix: String,
) {
    let filter_labels = Arc::clone(&labels);
    env.add_filter(
        "plural",
        move |value: Value, args: Rest<Value>, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
            plural_filter(
                &*filter_labels,
                runtime_text.as_deref(),
                &int_decimal_suffix,
                value,
                args,
                kwargs,
            )
        },
    );
}
