//! Opaque identifier names come from the owning plugin's live option sources.
use super::TYPES;
use leptos::prelude::*;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use ticknode_shared::*;

type Options = HashMap<(String, String), Arc<Vec<PropertyOption>>>;
thread_local! { static OPTIONS: RefCell<Options> = RefCell::new(HashMap::new()); }

pub fn set_identifier_options(options: &Options) {
    OPTIONS.with(|o| *o.borrow_mut() = options.clone());
    super::TRANSLATIONS_VERSION.with(|c| {
        if let Some(s) = c.get() {
            s.try_update(|n| *n += 1);
        }
    });
}

pub fn identifier_source(t: &DataType) -> Option<(String, String)> {
    TYPES.with(|types| {
        let types = types.borrow();
        let d = types.definition_of(t)?;
        let TypeKind::Identifier {
            labels: Some(source),
        } = &d.kind
        else {
            return None;
        };
        Some((d.plugin.clone()?, source.clone()))
    })
}

pub fn identifier_label(name: &str, id: i64) -> Option<String> {
    super::TRANSLATIONS_VERSION.with(|c| {
        if let Some(s) = c.get() {
            let _ = s.try_get();
        }
    });
    let (plugin, source) = identifier_source(&DataType::Named(name.into()))?;
    let value = id.to_string();
    OPTIONS.with(|all| {
        all.borrow()
            .get(&(plugin, source))?
            .iter()
            .find(|o| o.value == value)
            .filter(|o| !o.label.is_empty())
            .map(|o| o.label.clone())
    })
}

pub fn identifier_text(value: &DataValue, t: &DataType) -> Option<String> {
    let DataValue::Number(NumberValue::Int(id)) = value else {
        return None;
    };
    let DataType::Named(name) = t else {
        return None;
    };
    let identifier = TYPES.with(|types| {
        types
            .borrow()
            .get(name)
            .is_some_and(TypeDefinition::is_identifier)
    });
    identifier.then(|| {
        identifier_label(name, *id)
            .map_or_else(|| id.to_string(), |label| format!("{label} ({id})"))
    })
}

#[cfg(test)]
mod tests;
