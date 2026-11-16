//! Effective contracts for value trees, shared by execution and presentation.
use crate::{DataType, DataValue, TypeRegistry};

/// Resolves a dotted path using declarations first, then hidden field contracts and runtime kinds.
/// An empty path resolves the root; list elements are addressed by numeric indices.
pub fn effective_type(
    value: Option<&DataValue>,
    declared: &DataType,
    path: &str,
    types: &TypeRegistry,
) -> Option<DataType> {
    let mut value = value;
    let mut ty = refine(declared, value);
    if path.is_empty() {
        return Some(ty);
    }
    for segment in path.split('.') {
        let field_declared = types
            .definition_of(&ty)
            .and_then(|def| def.fields().iter().find(|f| f.name == segment))
            .map(|f| f.data_type.clone());
        match value {
            Some(DataValue::Object { fields, .. }) => {
                let field = fields.get(segment);
                let declared = field_declared.unwrap_or(DataType::Any);
                let contract = if matches!(declared, DataType::Any | DataType::Object) {
                    field.and_then(|f| f.data_type.clone()).unwrap_or(declared)
                } else {
                    declared
                };
                value = field.map(|f| &f.value);
                if value.is_none() && contract == DataType::Any {
                    return None;
                }
                ty = refine(&contract, value);
            }
            Some(DataValue::List {
                elements,
                element_type,
            }) => {
                let index = segment.parse::<usize>().ok()?;
                let contract = match &ty {
                    DataType::List(inner) => inner.as_ref(),
                    _ => element_type,
                };
                value = elements.get(index);
                ty = refine(contract, value);
            }
            _ => {
                value = None;
                ty = match ty {
                    DataType::List(inner) if segment.parse::<usize>().is_ok() => *inner,
                    _ => field_declared?,
                };
            }
        }
    }
    Some(ty)
}

fn refine(declared: &DataType, value: Option<&DataValue>) -> DataType {
    match (declared, value) {
        (DataType::Any | DataType::Object, Some(value)) => value.get_type(),
        (DataType::List(inner), Some(DataValue::List { element_type, .. }))
            if **inner == DataType::Any =>
        {
            DataType::List(Box::new(element_type.clone()))
        }
        _ => declared.clone(),
    }
}

/// Stamps known field contracts before a tree leaves a precise declaration (e.g. for an Any variable).
pub fn retain_field_types(value: &mut DataValue, declared: &DataType, types: &TypeRegistry) {
    let ty = effective_type(Some(value), declared, "", types).unwrap_or_else(|| declared.clone());
    match value {
        DataValue::Object { fields, .. } => {
            for (name, field) in fields {
                let declared = types
                    .definition_of(&ty)
                    .and_then(|d| d.fields().iter().find(|f| &f.name == name))
                    .map(|f| &f.data_type);
                let contract = declared
                    .filter(|t| !matches!(t, DataType::Any | DataType::Object))
                    .or(field.data_type.as_ref())
                    .or(declared)
                    .unwrap_or(&DataType::Any);
                let resolved = effective_type(Some(&field.value), contract, "", types)
                    .unwrap_or_else(|| contract.clone());
                retain_field_types(&mut field.value, &resolved, types);
                field.data_type = Some(resolved);
            }
        }
        DataValue::List {
            element_type,
            elements,
        } => {
            if let DataType::List(inner) = ty {
                *element_type = *inner;
            }
            for value in elements {
                retain_field_types(value, element_type, types);
            }
        }
        _ => {}
    }
}

/// On-demand field help: structure metadata wins, otherwise use the effective type's metadata.
pub fn field_hint(
    value: Option<&DataValue>,
    parent: &DataType,
    name: &str,
    types: &TypeRegistry,
) -> crate::UiMessage {
    let parent = effective_type(value, parent, "", types).unwrap_or_else(|| parent.clone());
    if let Some(def) = types.definition_of(&parent)
        && let Some(field) = def.fields().iter().find(|f| f.name == name)
        && !field.description.is_empty()
    {
        return crate::UiMessage::new(
            format!(
                "type-{}-field-{}-description",
                crate::fluent_slug(&def.name),
                field.name
            ),
            &field.description,
        );
    }
    let ty = effective_type(value, &parent, name, types).unwrap_or(DataType::Any);
    type_hint(&ty, types)
}

/// Type metadata for a field whose contract came from an on-demand catalog.
pub fn type_hint(ty: &DataType, types: &TypeRegistry) -> crate::UiMessage {
    if let Some(def) = types.definition_of(ty) {
        let (suffix, fallback) = if def.description.is_empty() {
            ("title", &def.title)
        } else {
            ("description", &def.description)
        };
        crate::UiMessage::new(
            format!("type-{}-{suffix}", crate::fluent_slug(&def.name)),
            fallback,
        )
    } else {
        let name = crate::data_type_name(ty);
        crate::UiMessage::new(format!("type-{}", crate::fluent_slug(&name)), name)
    }
}
