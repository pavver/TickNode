//! Catalog indexing, on-demand field requests and shared completion/lint contexts.
use super::*;
use ticknode_shared::plugin_values::editor::ValueTreeContext;

impl Editor {
    pub(crate) fn replace_value_catalog(&self, catalog: &ValueCatalog) {
        self.value_catalog_index
            .update(|tree| tree.replace(catalog));
        self.value_fields_pending.update_value(|pending| {
            pending.retain(|(namespace, _)| namespace != &catalog.namespace)
        });
    }

    pub(crate) fn with_value_tree<T>(&self, f: impl FnOnce(&ValueTreeContext) -> T) -> T {
        self.plugin_values.with(|values| {
            self.value_catalog_index.with(|catalog| {
                self.value_entry_fields.with(|fields| {
                    f(&ValueTreeContext {
                        catalog,
                        fields,
                        values,
                    })
                })
            })
        })
    }

    /// Requests only entries actually referenced in the draft; one request per cache miss.
    pub(crate) fn request_value_fields(&self, source: &str, template: bool) {
        let paths = ticknode_shared::plugin_values::lint::paths(source, template);
        let requests = self.value_catalog_index.with_untracked(|catalog| {
            paths
                .iter()
                .filter_map(|(path, _)| {
                    let path = path.strip_prefix("text.").unwrap_or(path);
                    let (namespace, entry, _) = catalog.entry(path)?;
                    Some((namespace.to_string(), entry.key.clone()))
                })
                .collect::<BTreeSet<_>>()
        });
        for (namespace, key) in requests {
            let entry = (namespace.clone(), key.clone());
            if self
                .value_entry_fields
                .with_untracked(|fields| fields.contains_key(&entry))
                || self
                    .value_fields_pending
                    .with_value(|pending| pending.contains(&entry))
            {
                continue;
            }
            self.value_fields_pending.update_value(|pending| {
                pending.insert(entry);
            });
            self.send(ClientMessage::ValueEntryFields { namespace, key });
        }
    }

    pub(crate) fn template_lints(
        &self,
        source: &str,
        inputs: &[(String, DataType)],
        variables: &[Variable],
    ) -> Vec<template::Lint> {
        self.request_value_fields(source, true);
        self.with_value_tree(|ctx| {
            self.types.with(|types| {
                template::lint::lint_with_catalog(source, inputs, types, variables, ctx)
            })
        })
    }

    pub(crate) fn formula_lints(
        &self,
        source: &str,
        inputs: &[(String, DataType)],
    ) -> Vec<formula::Lint> {
        self.request_value_fields(source, false);
        let mut inputs = inputs.to_vec();
        if let Ok(compiled) = formula::compile(source) {
            for reference in compiled.references() {
                if let Some(ty) = self.reference_type(&reference) {
                    inputs.push((reference.to_str(), ty));
                }
            }
        }
        self.with_value_tree(|ctx| {
            self.types
                .with(|types| formula::lint::lint_with_catalog(source, &inputs, types, ctx))
        })
    }

    pub(crate) fn reference_type(&self, reference: &VariableRef) -> Option<DataType> {
        if reference.lookup == VariableLookup::Plugin {
            self.request_value_fields(&reference.name, false);
            return self
                .with_value_tree(|ctx| {
                    self.types
                        .with(|types| ctx.field_type(&reference.name, types))
                })
                .or_else(|| self.plugin_value(&reference.name).map(|(_, ty)| ty));
        }
        self.variable_value(reference).map(|(_, ty)| ty)
    }

    /// Catalog labels remain available while an entry's live value is unavailable.
    pub(crate) fn binding_label(
        &self,
        reference: &VariableRef,
    ) -> Option<(String, Option<String>)> {
        if reference.lookup != VariableLookup::Plugin {
            return None;
        }
        self.value_catalog_index.with(|tree| {
            let (_, entry, suffix) = tree.entry(&reference.name)?;
            let title = if suffix.is_empty() {
                entry.title.clone()
            } else {
                format!("{} · {suffix}", entry.title)
            };
            Some((title, entry.icon.clone()))
        })
    }
}

/// A fetched field tree updates the currently focused completion without another keystroke.
pub(crate) fn refresh_completion(ed: Editor) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
        else {
            return;
        };
        if let Some(wrapper) = element.closest("[data-binding-node]").ok().flatten() {
            let node = wrapper
                .get_attribute("data-binding-node")
                .and_then(|s| s.parse().ok())
                .map(NodeId);
            let pin = wrapper
                .get_attribute("data-binding-pin")
                .and_then(|s| s.parse().ok())
                .map(PinId);
            if let (Some(node), Some(pin)) = (node, pin) {
                crate::inline::binding_completion::request(ed, node, pin, &element);
            }
        } else if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            let id = textarea.id();
            if let Some(id) = id
                .strip_prefix("template-textarea-")
                .and_then(|s| s.parse().ok())
                .map(NodeId)
            {
                crate::inline::template_field::completion::request_completion(ed, id, false);
            } else if let Some(id) = id
                .strip_prefix("formula-textarea-")
                .and_then(|s| s.parse().ok())
                .map(NodeId)
            {
                crate::inline::formula_field::completion::request_completion(ed, id, false);
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ed;
}

#[cfg(test)]
mod tests;
