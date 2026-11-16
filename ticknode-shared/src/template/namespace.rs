//! Nested namespace maps, including per-field presentation contracts.
use super::{RenderEnv, values};
use crate::{DataType, DataValue};
use minijinja::{Environment, Value};
use std::collections::BTreeMap;

#[derive(Default)]
struct Tree {
    value: Option<Value>,
    children: BTreeMap<String, Tree>,
}
impl Tree {
    fn insert(&mut self, path: &str, value: Value) {
        let mut tree = self;
        for part in path.split('.') {
            tree = tree.children.entry(part.into()).or_default();
        }
        tree.value = Some(value);
    }
    fn into_value(self) -> Value {
        if self.children.is_empty() {
            self.value.unwrap_or(Value::UNDEFINED)
        } else {
            Value::from_iter(self.children.into_iter().map(|(k, v)| (k, v.into_value())))
        }
    }
}
fn presentation(
    value: &DataValue,
    path: &str,
    ctx: &RenderEnv,
    format: &dyn Fn(&DataValue, &DataType) -> String,
) -> Value {
    match value {
        DataValue::Object { fields, .. } => fields
            .iter()
            .map(|(key, field)| {
                (
                    key.clone(),
                    presentation(&field.value, &format!("{path}.{key}"), ctx, format),
                )
            })
            .collect(),
        DataValue::List { elements, .. } => elements
            .iter()
            .enumerate()
            .map(|(i, value)| presentation(value, &format!("{path}.{i}"), ctx, format))
            .collect(),
        _ => {
            let ty = ctx
                .plugin_values
                .iter()
                .find(|(p, _, _)| p == path)
                .map(|(_, _, ty)| ty.clone())
                .unwrap_or_else(|| value.get_type());
            Value::from(format(value, &ty))
        }
    }
}
pub(super) fn register(
    env: &mut Environment<'static>,
    ctx: &RenderEnv,
    text: &mut Vec<(String, Value)>,
    format: &dyn Fn(&DataValue, &DataType) -> String,
) {
    for namespace in &ctx.namespaces {
        let mut plain = Tree::default();
        let mut formatted = Tree::default();
        let mut parents: Vec<String> = Vec::new();
        for (full, value, _) in &ctx.plugin_values {
            if parents.iter().any(|p| full.starts_with(&format!("{p}."))) {
                continue;
            }
            if let Some(path) = full.strip_prefix(&format!("{namespace}.")) {
                parents.push(full.clone());
                if let Some(value) = value {
                    plain.insert(path, values::plain(value));
                    formatted.insert(path, presentation(value, full, ctx, format));
                }
            }
        }
        env.add_global(namespace.clone(), plain.into_value());
        text.push((namespace.clone(), formatted.into_value()));
    }
}

#[cfg(test)]
mod tests;
