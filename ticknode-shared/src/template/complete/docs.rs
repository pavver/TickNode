//! Documentation catalog for template filters, tests, and functions.

use crate::UiMessage;
use crate::template::complete::CompletionKind;

mod filters;
mod tests_fn;

pub use filters::FILTER_DOCS;
pub use tests_fn::{FUNCTION_DOCS, TEST_DOCS};

/// Kind of template item documented in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKind {
    Filter,
    Test,
    Function,
}

impl DocKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            DocKind::Filter => "filter",
            DocKind::Test => "test",
            DocKind::Function => "function",
        }
    }
}

impl From<DocKind> for CompletionKind {
    fn from(kind: DocKind) -> Self {
        match kind {
            DocKind::Filter => CompletionKind::Filter,
            DocKind::Test => CompletionKind::Test,
            DocKind::Function => CompletionKind::Function,
        }
    }
}

/// Static documentation entry for a filter, test, or function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Doc {
    pub name: &'static str,
    pub kind: DocKind,
    pub signature: &'static str,
    pub params: &'static [(&'static str, &'static str)],
    pub help_key: &'static str,
    pub help_fallback: &'static str,
}

impl Doc {
    pub fn help(&self) -> UiMessage {
        UiMessage::new(self.help_key, self.help_fallback)
    }

    pub fn param_help(&self, param_name: &str) -> Option<UiMessage> {
        self.params
            .iter()
            .find(|(name, _)| *name == param_name)
            .map(|(name, english)| {
                let key = param_help_key(self.kind, self.name, name);
                UiMessage::new(key, *english)
            })
    }
}

/// Formats the localization key for a parameter: `template-help-param-<filter|test|function>-<name>-<param>`.
pub fn param_help_key(kind: DocKind, name: &str, param: &str) -> String {
    format!("template-help-param-{}-{name}-{param}", kind.as_str())
}

/// Looks up the documentation for a named item of the given kind.
pub fn find_doc(kind: DocKind, name: &str) -> Option<&'static Doc> {
    let slice = match kind {
        DocKind::Filter => FILTER_DOCS,
        DocKind::Test => TEST_DOCS,
        DocKind::Function => FUNCTION_DOCS,
    };
    slice.iter().find(|d| d.name == name)
}

/// Iterator over all documentation entries across all kinds.
pub fn all_docs() -> impl Iterator<Item = &'static Doc> {
    FILTER_DOCS
        .iter()
        .chain(TEST_DOCS.iter())
        .chain(FUNCTION_DOCS.iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::complete::catalog::{BUILTIN_FILTERS, FUNCTIONS, TESTS};

    #[test]
    fn every_builtin_filter_has_a_doc() {
        for &(name, _) in BUILTIN_FILTERS {
            let doc = find_doc(DocKind::Filter, name);
            assert!(doc.is_some(), "missing Doc for BUILTIN_FILTER '{name}'");
        }
        assert!(
            find_doc(DocKind::Filter, "text").is_some(),
            "missing Doc for 'text' filter"
        );
    }

    #[test]
    fn every_test_has_a_doc() {
        for &name in TESTS {
            let doc = find_doc(DocKind::Test, name);
            assert!(doc.is_some(), "missing Doc for TEST '{name}'");
        }
    }

    #[test]
    fn every_function_has_a_doc() {
        for &(name, _) in FUNCTIONS {
            let doc = find_doc(DocKind::Function, name);
            assert!(doc.is_some(), "missing Doc for FUNCTION '{name}'");
        }
    }

    #[test]
    fn all_docs_have_consistent_keys() {
        for doc in all_docs() {
            let expected_prefix = format!("template-help-{}-{}", doc.kind.as_str(), doc.name);
            assert_eq!(
                doc.help_key, expected_prefix,
                "Doc for {} has mismatched help_key",
                doc.name
            );
            assert!(!doc.help_fallback.is_empty());
            for &(param_name, fallback) in doc.params {
                let pkey = param_help_key(doc.kind, doc.name, param_name);
                assert!(
                    pkey.starts_with("template-help-param-"),
                    "Param key '{pkey}' invalid"
                );
                assert!(!fallback.is_empty());
            }
        }
    }
}
