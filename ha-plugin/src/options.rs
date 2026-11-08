//! Naming policy for global and entity-specific option sources.
pub fn entity_source(source: &str, entity: &str) -> String {
    let entity = entity.trim();
    if entity.is_empty() {
        source.into()
    } else {
        format!("{source}:{entity}")
    }
}
