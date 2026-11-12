//! Live option lookup for opaque identifiers used by native text nodes.
use super::OptionStore;
use crate::TickEngine;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use ticknode_shared::{TypeKind, to_text::IdentifierLabels};

#[derive(Clone, Default)]
pub(crate) struct Labels(HashMap<String, (String, Arc<Mutex<OptionStore>>)>);

impl IdentifierLabels for Labels {
    fn label(&self, name: &str, id: i64) -> Option<String> {
        let (source, store) = self.0.get(name)?;
        let store = store.lock().unwrap_or_else(|p| p.into_inner());
        let value = id.to_string();
        store
            .sources
            .get(source)?
            .iter()
            .find(|o| o.value == value)
            .filter(|o| !o.label.is_empty())
            .map(|o| o.label.clone())
    }
}

impl TickEngine {
    pub(crate) fn refresh_identifier_labels(&mut self) {
        self.host_state.identifier_labels = Labels(
            self.types
                .definitions()
                .into_iter()
                .filter_map(|d| {
                    let TypeKind::Identifier {
                        labels: Some(source),
                    } = d.kind
                    else {
                        return None;
                    };
                    let plugin = self.plugins.get(d.plugin.as_deref()?)?;
                    let options = plugin
                        .runtime
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .options
                        .clone();
                    Some((d.name, (source, options)))
                })
                .collect(),
        );
        self.pure_cache.clear();
    }
}
