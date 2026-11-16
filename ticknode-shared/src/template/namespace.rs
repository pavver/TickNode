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
pub(super) fn register(
    env: &mut Environment<'static>,
    ctx: &RenderEnv,
    text: &mut Vec<(String, Value)>,
    path_types: &std::collections::HashMap<String, DataType>,
    types: &crate::TypeRegistry,
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
                    formatted.insert(
                        path,
                        super::formatted::value(value, full, path_types, types, format),
                    );
                }
            }
        }
        env.add_global(namespace.clone(), plain.into_value());
        text.push((namespace.clone(), formatted.into_value()));
    }
}

#[cfg(test)]
mod tests;
