use ticknode_shared::*;
use super::wit;

/// Options of dynamic properties published by a plugin (`host::set-options`). It outlives plugin instances
/// (a crash does not clear it) and is cleared when the plugin is disabled.
#[derive(Default)]
pub(crate) struct OptionStore {
    pub(super) sources: std::collections::BTreeMap<String, Vec<PropertyOption>>,
    /// Sources whose list changed since the last `take_changes`.
    dirty: std::collections::BTreeSet<String>,
}

impl OptionStore {
    /// Replaces a source. Returns `false` and keeps everything when the list is unchanged.
    pub(super) fn set(&mut self, source: &str, options: Vec<PropertyOption>) -> bool {
        let unchanged = match self.sources.get(source) {
            Some(old) => *old == options,
            None => options.is_empty(),
        };
        if unchanged {
            return false;
        }
        if options.is_empty() {
            self.sources.remove(source);
        } else {
            self.sources.insert(source.to_string(), options);
        }
        self.dirty.insert(source.to_string());
        true
    }

    /// Removes every source (the plugin was disabled); editors are told through the dirty set.
    pub(super) fn clear(&mut self) {
        let keys: Vec<String> = self.sources.keys().cloned().collect();
        self.sources.clear();
        self.dirty.extend(keys);
    }

    pub(super) fn take_changes(&mut self, plugin: &str) -> Vec<PropertyOptionSet> {
        std::mem::take(&mut self.dirty).into_iter()
            .map(|source| {
                let options = self.sources.get(&source).cloned().unwrap_or_default();
                PropertyOptionSet { plugin: plugin.to_string(), source, options }
            })
            .collect()
    }

    pub(super) fn all(&self, plugin: &str) -> Vec<PropertyOptionSet> {
        self.sources.iter()
            .map(|(source, options)| PropertyOptionSet { plugin: plugin.to_string(), source: source.clone(), options: options.clone() })
            .collect()
    }
}

/// Applies the host limits to options published by a plugin. Returns the accepted options and English
/// warnings for what was dropped.
pub(super) fn sanitize_options(source_count_ok: bool, source: &str, options: Vec<wit::PropertyOption>) -> (Vec<PropertyOption>, Vec<String>) {
    let mut warnings = Vec::new();
    if source.is_empty() || source.len() > MAX_OPTION_STRING_LEN {
        return (Vec::new(), vec!["Option source name is empty or too long: ignored".to_string()]);
    }
    if !source_count_ok {
        return (Vec::new(), vec![format!("Too many option sources (maximum {MAX_OPTION_SOURCES}): source \"{source}\" ignored")]);
    }
    let mut options = options;
    if options.len() > MAX_OPTIONS_PER_SOURCE {
        warnings.push(format!("Source \"{source}\" has {} options, keeping the first {MAX_OPTIONS_PER_SOURCE}", options.len()));
        options.truncate(MAX_OPTIONS_PER_SOURCE);
    }
    let too_long = |s: &str| s.len() > MAX_OPTION_STRING_LEN;
    let mut dropped = 0usize;
    let accepted = options.into_iter().filter_map(|o| {
        if o.value.is_empty() || too_long(&o.value) || too_long(&o.label) || too_long(&o.detail) || o.icon.as_deref().is_some_and(too_long) {
            dropped += 1;
            return None;
        }
        Some(PropertyOption { value: o.value, label: o.label, detail: o.detail, icon: o.icon })
    }).collect();
    if dropped > 0 {
        warnings.push(format!("Source \"{source}\": dropped {dropped} option(s) with an empty value or a string longer than {MAX_OPTION_STRING_LEN} bytes"));
    }
    (accepted, warnings)
}
