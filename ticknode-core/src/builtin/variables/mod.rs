use ticknode_shared::*;

mod get;
mod set;
mod create;
mod delete;
mod changed;

pub use get::*;
pub use set::*;
pub use create::*;
pub use delete::*;
pub use changed::*;

pub(super) const LOOKUP_OPTIONS: &[(&str, &str)] = &[
    ("auto", "Auto"),
    ("sheet", "Sheet"),
    ("global", "Global"),
];

pub(super) const SCOPE_OPTIONS: &[(&str, &str)] = &[
    ("sheet", "Sheet"),
    ("global", "Global"),
    ("persistent", "Persistent"),
];

pub(super) fn parse_lookup(text: &str) -> VariableLookup {
    match text.trim().to_ascii_lowercase().as_str() {
        "sheet" => VariableLookup::Sheet,
        "global" => VariableLookup::Global,
        _ => VariableLookup::Auto,
    }
}

pub(super) fn format_lookup(lookup: VariableLookup) -> &'static str {
    match lookup {
        VariableLookup::Auto => "auto",
        VariableLookup::Sheet => "sheet",
        VariableLookup::Global => "global",
    }
}

pub(super) fn parse_scope(text: &str) -> VariableScope {
    match text.trim().to_ascii_lowercase().as_str() {
        "global" => VariableScope::Global,
        "persistent" => VariableScope::Persistent,
        _ => VariableScope::Sheet,
    }
}

pub(super) fn format_scope(scope: VariableScope) -> &'static str {
    match scope {
        VariableScope::Sheet => "sheet",
        VariableScope::Global => "global",
        VariableScope::Persistent => "persistent",
    }
}
