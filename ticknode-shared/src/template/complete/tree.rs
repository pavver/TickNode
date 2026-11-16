//! Completion of declared and self-describing object fields on the same paths.
use super::*;
use crate::{
    DataValue, VariableScope, effective_type, plugin_values::project, value_tree::field_hint,
};
use std::collections::BTreeSet;

pub(super) fn dot_items(
    before: &str,
    partial: &str,
    input: &CompletionInput,
    values: &[(String, Option<DataValue>, DataType)],
) -> Option<Vec<CompletionItem>> {
    let path = scan::parse_dotted_path(before.strip_suffix('.')?)?;
    let path = if path.first().is_some_and(|p| p == "text") {
        &path[1..]
    } else {
        &path[..]
    };
    let full = path.join(".");
    let (value, declared, suffix) = if let Some((name, value, ty)) = values
        .iter()
        .filter(|(name, _, _)| full == *name || full.starts_with(&format!("{name}.")))
        .max_by_key(|(name, _, _)| name.len())
    {
        (
            value.as_ref(),
            ty.clone(),
            full.strip_prefix(name)?.trim_start_matches('.').to_string(),
        )
    } else if path.len() >= 2 && ["var", "sheet", "global"].contains(&path[0].as_str()) {
        let scope = &path[0];
        let var = input
            .variables
            .iter()
            .filter(|v| v.key.name == path[1])
            .filter(|v| match scope.as_str() {
                "sheet" => v.key.scope == VariableScope::Sheet,
                "global" => v.key.scope != VariableScope::Sheet,
                _ => true,
            })
            .max_by_key(|v| v.key.scope == VariableScope::Sheet)?;
        (Some(&var.value), var.data_type.clone(), path[2..].join("."))
    } else {
        let (name, ty) = input
            .inputs
            .iter()
            .find(|(name, _)| path.first() == Some(name))?;
        (
            None,
            ty.clone(),
            full.strip_prefix(name)?.trim_start_matches('.').to_string(),
        )
    };
    let ty = effective_type(value, &declared, &suffix, input.types)?;
    let value = value.and_then(|v| project(v, &suffix));
    let mut names: BTreeSet<String> = input
        .types
        .definition_of(&ty)
        .map(|d| d.fields().iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    if let Some(DataValue::Object { fields, .. }) = value {
        names.extend(fields.keys().cloned());
    }
    if names.is_empty() {
        return None;
    }
    let items = names
        .into_iter()
        .filter_map(|name| {
            let field_type = effective_type(value, &ty, &name, input.types)?;
            let detail = if let Some(labels) = input.labels {
                let key = match &field_type {
                    DataType::Named(n) => format!("type-{}-title", crate::fluent_slug(n)),
                    _ => format!("type-{}", crate::data_type_name(&field_type)),
                };
                labels
                    .message(&key)
                    .unwrap_or_else(|| type_label(&field_type, input.types))
            } else {
                type_label(&field_type, input.types)
            };
            Some(CompletionItem {
                label: name.clone(),
                insert: name.clone(),
                kind: CompletionKind::Field,
                detail,
                help: Some(field_hint(value, &ty, &name, input.types)),
                caret: None,
            })
        })
        .collect();
    Some(filter_and_dedup(items, partial))
}
