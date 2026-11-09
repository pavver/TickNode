//! Pure quick-add ranking and recent-type updates, independent of browser state.

use ticknode_shared::NodeDefinition;
use crate::categories::category_rank;

pub const MAX_RECENT: usize = 5;

pub fn update_recent(recent: &[String], added: &str) -> Vec<String> {
    let mut result = Vec::new();
    for name in std::iter::once(added).chain(recent.iter().map(String::as_str)) {
        if !name.is_empty() && !result.iter().any(|n| n == name) {
            result.push(name.to_string());
            if result.len() == MAX_RECENT { break; }
        }
    }
    result
}

#[derive(Clone, PartialEq)]
pub struct Choice {
    pub index: usize,
    pub title: String,
    pub category: String,
    pub recent: bool,
}

/// Every search term must match; ranks retain palette order for ties.
/// Recent entries are promoted once, so the list contains every available type exactly once.
pub fn ranked(
    catalog: &[NodeDefinition],
    recent: &[String],
    query: &str,
    labels: impl Fn(&NodeDefinition) -> (String, String, String),
) -> Vec<Choice> {
    let query = query.trim().to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let mut choices: Vec<_> = catalog.iter().enumerate().filter(|(_, d)| d.category != "Unavailable")
        .filter_map(|(index, def)| {
            let (title, category, description) = labels(def);
            let lower = title.to_lowercase();
            let haystack = format!("{lower} {category} {description} {}", def.type_name).to_lowercase();
            if !terms.iter().all(|term| haystack.contains(term)) { return None; }
            let rank = if lower.starts_with(&query) { 0 }
                else if terms.iter().all(|term| lower.contains(term)) { 1 } else { 2 };
            Some((rank, Choice { index, title, category, recent: false }))
        }).collect();
    choices.sort_by(|(a_rank, a), (b_rank, b)| {
        let a_def = &catalog[a.index];
        let b_def = &catalog[b.index];
        (a_rank, category_rank(&a_def.category), &a_def.category, &a.title, &a_def.type_name)
            .cmp(&(b_rank, category_rank(&b_def.category), &b_def.category, &b.title, &b_def.type_name))
    });
    let mut choices: Vec<_> = choices.into_iter().map(|(_, c)| c).collect();
    if terms.is_empty() {
        let mut promoted = Vec::new();
        for name in recent.iter().take(MAX_RECENT) {
            if let Some(at) = choices.iter().position(|c| catalog[c.index].type_name == *name) {
                let mut choice = choices.remove(at);
                choice.recent = true;
                promoted.push(choice);
            }
        }
        promoted.extend(choices);
        choices = promoted;
    }
    choices
}

#[cfg(test)]
mod tests;
