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
    let items = field_items(value, &ty, input, partial, &[]);
    (!items.is_empty()).then_some(items)
}

/// Shared completion for declared structure fields, runtime metadata and fetched catalog fields.
fn field_items(
    value: Option<&DataValue>,
    ty: &DataType,
    input: &CompletionInput,
    partial: &str,
    extra: &[(String, DataType)],
) -> Vec<CompletionItem> {
    let mut names: BTreeSet<String> = input
        .types
        .definition_of(ty)
        .map(|d| d.fields().iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    if let Some(DataValue::Object { fields, .. }) = value {
        names.extend(fields.keys().cloned());
    }
    names.extend(extra.iter().map(|(name, _)| name.clone()));
    names
        .into_iter()
        .filter(|name| name.starts_with(partial))
        .take(50)
        .filter_map(|name| {
            let declared = extra.iter().find(|(n, _)| *n == name).map(|(_, ty)| ty);
            let field_type = if let Some(declared) = declared {
                effective_type(
                    value.and_then(|v| project(v, &name)),
                    declared,
                    "",
                    input.types,
                )?
            } else {
                effective_type(value, ty, &name, input.types)?
            };
            Some(CompletionItem {
                label: name.clone(),
                insert: name.clone(),
                icon: None,
                kind: CompletionKind::Field,
                detail: localized_type_label(&field_type, input),
                help: Some(
                    if declared.is_some() && input.types.definition_of(ty).is_none() {
                        crate::value_tree::type_hint(&field_type, input.types)
                    } else {
                        field_hint(value, ty, &name, input.types)
                    },
                ),
                caret: None,
            })
        })
        .collect()
}

use crate::plugin_values::editor::ValueTreeContext;

pub(crate) fn namespace_roots(ctx: &ValueTreeContext, partial: &str) -> Vec<CompletionItem> {
    ctx.catalog
        .roots
        .keys()
        .filter(|name| name.starts_with(partial))
        .take(50)
        .map(|name| CompletionItem {
            label: name.clone(),
            insert: format!("{name}."),
            icon: None,
            kind: CompletionKind::Variable,
            detail: String::new(),
            help: None,
            caret: None,
        })
        .collect()
}

pub(crate) fn namespace_items(
    before: &str,
    partial: &str,
    input: &CompletionInput,
    ctx: &ValueTreeContext,
) -> Option<Vec<CompletionItem>> {
    let path = scan::parse_dotted_path(before.strip_suffix('.')?)?;
    let path = if path.first().is_some_and(|s| s == "text") {
        &path[1..]
    } else {
        &path[..]
    };
    if path.is_empty() {
        return None;
    }
    if !ctx.catalog.roots.contains_key(&path[0])
        || input.inputs.iter().any(|(name, _)| *name == path[0])
    {
        return None;
    }
    let full = path.join(".");
    if let Some(branch) = ctx.catalog.branch(&full)
        && !branch.children.is_empty()
    {
        return Some(
            branch
                .children
                .range(partial.to_string()..)
                .take_while(|(name, _)| name.starts_with(partial))
                .take(50)
                .map(|(name, branch)| CompletionItem {
                    label: name.clone(),
                    insert: format!("{name}."),
                    kind: CompletionKind::Field,
                    detail: branch
                        .entry
                        .as_ref()
                        .map(|e| e.title.clone())
                        .unwrap_or_default(),
                    icon: branch.entry.as_ref().and_then(|e| e.icon.clone()),
                    help: None,
                    caret: None,
                })
                .collect(),
        );
    }
    let Some((namespace, entry, suffix)) = ctx.catalog.entry(&full) else {
        return Some(Vec::new());
    };
    let Some(ty) = ctx.field_type(&full, input.types) else {
        return Some(Vec::new());
    };
    let value = ctx
        .value(namespace, &entry.key)
        .and_then(|v| project(v, suffix));
    let mut extra = std::collections::BTreeMap::new();
    if let Some(fields) = ctx.fields(namespace, &entry.key) {
        let prefix = if suffix.is_empty() {
            String::new()
        } else {
            format!("{suffix}.")
        };
        for field in fields {
            if let Some(tail) = field.path.strip_prefix(&prefix) {
                let name = tail.split('.').next().unwrap_or(tail);
                if name.starts_with(partial) {
                    let path = format!("{full}.{name}");
                    if let Some(ty) = ctx.field_type(&path, input.types) {
                        extra.insert(name.to_string(), ty);
                    }
                }
            }
        }
    }
    Some(field_items(
        value,
        &ty,
        input,
        partial,
        &extra.into_iter().collect::<Vec<_>>(),
    ))
}
