//! Pure catalog matching for connections dropped on the canvas background.

use std::collections::HashMap;
use ticknode_shared::{DataType, NodeDefinition, PinDirection, PinId, PinType, TypeRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub type_name: String,
    pub title: String,
    pub category: String,
    /// Untranslated definition category: groups follow palette order.
    pub group: String,
    pub pin: PinId,
    pub rank: u8,
}

fn rank(registry: &TypeRegistry, src: &PinType, dst: &PinType) -> Option<u8> {
    match (src, dst) {
        (PinType::Exec, PinType::Exec) => Some(0),
        (PinType::Data(src), PinType::Data(dst)) => {
            if *src == DataType::Any || *dst == DataType::Any {
                Some(3)
            } else if src == dst {
                Some(0)
            } else if registry.compatible(src, dst) {
                Some(1)
            } else {
                registry.convertible(src, dst).map(|_| 2)
            }
        }
        _ => None,
    }
}

/// Labels are supplied by the caller so ranking stays independent of the UI and locale state.
/// Catalogs contain only available definitions; inferred unknown stubs use `Unavailable`.
pub fn suggestions(
    direction: PinDirection,
    pin_type: &PinType,
    catalog: &[NodeDefinition],
    registry: &TypeRegistry,
    labels: impl Fn(&NodeDefinition) -> (String, String),
) -> Vec<Suggestion> {
    let mut result = Vec::new();
    for definition in catalog {
        if definition.category == "Unavailable" { continue; }
        let best = definition.pins.iter().filter(|p| p.direction != direction).filter_map(|p| {
            let rank = if definition.adaptive && matches!((pin_type, &p.pin_type), (PinType::Data(_), PinType::Data(_))) {
                Some(0)
            } else if direction == PinDirection::Output {
                rank(registry, pin_type, &p.pin_type)
            } else {
                rank(registry, &p.pin_type, pin_type)
            }?;
            Some((rank, p.id.0))
        }).min();
        if let Some((rank, pin)) = best {
            let (title, category) = labels(definition);
            result.push(Suggestion {
                type_name: definition.type_name.clone(), title, category,
                group: definition.category.clone(), pin: PinId(pin), rank,
            });
        }
    }
    result.sort_by(|a, b| (a.rank, &a.title, &a.type_name).cmp(&(b.rank, &b.title, &b.type_name)));
    result
}

/// How many suggestions the "frequently used" section shows at most.
pub const MAX_POPULAR: usize = 5;

/// Menu order: up to [`MAX_POPULAR`] frequently used types first (most nodes of that type in the
/// graph, then most recently added), then every other suggestion grouped by category in palette
/// order and sorted by title. Each suggestion appears once; `true` marks the popular ones.
pub fn arrange(
    list: Vec<Suggestion>,
    usage: &HashMap<String, usize>,
    recent: &[String],
    group_rank: impl Fn(&str) -> usize,
) -> Vec<(bool, Suggestion)> {
    let recency = |name: &str| recent.iter().position(|r| r == name).unwrap_or(usize::MAX);
    let mut popular: Vec<_> = list.iter().enumerate()
        .map(|(i, s)| (usage.get(&s.type_name).copied().unwrap_or(0), recency(&s.type_name), i))
        .filter(|&(count, at, _)| count > 0 || at != usize::MAX)
        .collect();
    popular.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    popular.truncate(MAX_POPULAR);
    let picked: Vec<usize> = popular.iter().map(|&(_, _, i)| i).collect();
    let mut rest: Vec<_> = list.iter().enumerate().filter(|(i, _)| !picked.contains(i)).map(|(_, s)| s).collect();
    rest.sort_by(|a, b| (group_rank(&a.group), &a.group, &a.title, &a.type_name)
        .cmp(&(group_rank(&b.group), &b.group, &b.title, &b.type_name)));
    picked.iter().map(|&i| (true, list[i].clone()))
        .chain(rest.into_iter().map(|s| (false, s.clone())))
        .collect()
}

#[cfg(test)]
mod tests;
