use crate::{NodeDefinition, PinDirection, PinId, PinType};
use serde::{Deserialize, Serialize};

/// Reuse the pin definition's execution/data distinction in persisted signatures.
pub type PinKind = PinType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinSignature {
    pub id: PinId,
    pub direction: PinDirection,
    pub kind: PinKind,
    pub required: bool,
    /// Display only; renaming a pin does not change its contract.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PinChange {
    Removed {
        pin: PinSignature,
    },
    Changed {
        old: PinSignature,
        new: PinSignature,
    },
    RequiredAdded {
        pin: PinSignature,
    },
}

/// Call with the base definition, before expanding split struct pins.
pub fn pin_signature(definition: &NodeDefinition) -> Vec<PinSignature> {
    definition
        .pins
        .iter()
        .map(|p| PinSignature {
            id: p.id,
            direction: p.direction,
            kind: p.pin_type.clone(),
            required: p.required,
            name: p.name.clone(),
        })
        .collect()
}

/// Compares contracts independently of display names and pin ordering.
pub fn pin_changes(old: &[PinSignature], new: &[PinSignature]) -> (Vec<PinChange>, bool) {
    let mut breaking = Vec::new();
    let mut different = old.len() != new.len();
    for pin in old {
        let current = new
            .iter()
            .find(|n| n.id == pin.id && n.direction == pin.direction)
            .or_else(|| new.iter().find(|n| n.id == pin.id));
        match current {
            None => {
                different = true;
                breaking.push(PinChange::Removed { pin: pin.clone() });
            }
            Some(n) if n.direction != pin.direction || n.kind != pin.kind => {
                different = true;
                breaking.push(PinChange::Changed {
                    old: pin.clone(),
                    new: n.clone(),
                });
            }
            Some(n) => {
                different |= n.required != pin.required;
            }
        }
    }
    for pin in new {
        let previous = old
            .iter()
            .find(|p| p.id == pin.id && p.direction == pin.direction);
        different |= previous.is_none();
        if pin.direction == PinDirection::Input
            && pin.required
            && previous.is_none_or(|p| !p.required)
        {
            breaking.push(PinChange::RequiredAdded { pin: pin.clone() });
        }
    }
    (breaking, different)
}

#[cfg(test)]
mod tests;
