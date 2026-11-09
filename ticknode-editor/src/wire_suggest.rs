//! Pure catalog matching for connections dropped on the canvas background.

use ticknode_shared::{DataType, NodeDefinition, PinDirection, PinId, PinType, TypeRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub type_name: String,
    pub title: String,
    pub category: String,
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
            let rank = if direction == PinDirection::Output {
                rank(registry, pin_type, &p.pin_type)
            } else {
                rank(registry, &p.pin_type, pin_type)
            }?;
            Some((rank, p.id.0))
        }).min();
        if let Some((rank, pin)) = best {
            let (title, category) = labels(definition);
            result.push(Suggestion { type_name: definition.type_name.clone(), title, category, pin: PinId(pin), rank });
        }
    }
    result.sort_by(|a, b| (a.rank, &a.title, &a.type_name).cmp(&(b.rank, &b.title, &b.type_name)));
    result
}

#[cfg(test)]
mod tests;
