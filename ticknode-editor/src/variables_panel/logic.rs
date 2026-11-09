//! Pure logic for variables grouping, sorting, shadowing, and search filtering.

use std::collections::HashSet;
use ticknode_shared::{SheetId, TypeRegistry, Variable, VariableScope, data_type_name, text};
use crate::i18n;

#[derive(Debug, Clone, PartialEq)]
pub struct VariableItem {
    pub variable: Variable,
    pub shadowed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariableGroups {
    pub sheet: Vec<VariableItem>,
    pub global: Vec<VariableItem>,
    pub persistent: Vec<VariableItem>,
}

impl VariableGroups {
    pub fn is_empty(&self) -> bool {
        self.sheet.is_empty() && self.global.is_empty() && self.persistent.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.sheet.len() + self.global.len() + self.persistent.len()
    }
}

/// Groups variables by scope (Sheet, Global, Persistent), applies case-insensitive sorting by name,
/// computes shadowing for global/persistent variables on the active sheet, and filters by search query.
pub fn group_and_filter_variables(
    variables: &[Variable],
    sheet: Option<SheetId>,
    query: &str,
    types: &TypeRegistry,
) -> VariableGroups {
    let sheet_names: HashSet<&str> = if let Some(sheet_id) = sheet {
        variables
            .iter()
            .filter(|v| v.key.scope == VariableScope::Sheet && v.key.sheet == Some(sheet_id))
            .map(|v| v.key.name.as_str())
            .collect()
    } else {
        HashSet::new()
    };

    let query = query.trim().to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();

    let mut groups = VariableGroups::default();

    for v in variables {
        let is_visible = match v.key.scope {
            VariableScope::Sheet => sheet.is_some() && v.key.sheet == sheet,
            VariableScope::Global | VariableScope::Persistent => true,
        };
        if !is_visible {
            continue;
        }

        if !terms.is_empty() {
            let name = v.key.name.to_lowercase();
            let type_title = i18n::data_type_title(types, &v.data_type).to_lowercase();
            let type_name = data_type_name(&v.data_type).to_lowercase();
            let val_text = text::format_value(&v.value).to_lowercase();
            let haystack = format!("{name} {type_title} {type_name} {val_text}");

            let matches = terms.iter().all(|term| haystack.contains(term));
            if !matches {
                continue;
            }
        }

        let shadowed = sheet.is_some()
            && matches!(v.key.scope, VariableScope::Global | VariableScope::Persistent)
            && sheet_names.contains(v.key.name.as_str());

        let item = VariableItem {
            variable: v.clone(),
            shadowed,
        };

        match v.key.scope {
            VariableScope::Sheet => groups.sheet.push(item),
            VariableScope::Global => groups.global.push(item),
            VariableScope::Persistent => groups.persistent.push(item),
        }
    }

    let sort_items = |items: &mut Vec<VariableItem>| {
        items.sort_by(|a, b| {
            a.variable.key.name.to_lowercase()
                .cmp(&b.variable.key.name.to_lowercase())
                .then_with(|| a.variable.key.name.cmp(&b.variable.key.name))
        });
    };

    sort_items(&mut groups.sheet);
    sort_items(&mut groups.global);
    sort_items(&mut groups.persistent);

    groups
}

#[cfg(test)]
mod tests;
