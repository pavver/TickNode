use super::*;
use crate::{DataType, PinDefinition};

fn pin(id: u32) -> PinSignature {
    PinSignature { id: PinId(id), direction: PinDirection::Input, kind: PinKind::Data(DataType::Number), required: false, name: "Value".into() }
}

#[test]
fn base_definition_signature_keeps_order_and_display_names() {
    let pins = vec![pin(2), pin(1)];
    let definition = NodeDefinition {
        type_name: "test".into(), title: "Test".into(), category: "Test".into(),
        description: String::new(), help: String::new(), properties: Vec::new(), cost: 1,
        pins: pins.iter().map(|p| PinDefinition {
            id: p.id, direction: p.direction, pin_type: p.kind.clone(), required: p.required, name: p.name.clone(),
        }).collect(),
    };
    assert_eq!(pin_signature(&definition), pins);
}

#[test]
fn compares_contracts_ignoring_names_and_order() {
    let old = vec![pin(1), pin(2)];
    let mut new = vec![pin(2), pin(1)];
    new[0].name = "Renamed".into();
    assert_eq!(pin_changes(&old, &new), (vec![], false));
    new.push(pin(3));
    assert_eq!(pin_changes(&old, &new), (vec![], true));
    new[2].required = true;
    assert_eq!(pin_changes(&old, &new).0, vec![PinChange::RequiredAdded { pin: new[2].clone() }]);
    new[2].direction = PinDirection::Output;
    assert!(pin_changes(&old, &new).0.is_empty());
}

#[test]
fn removed_changed_and_required_transitions() {
    let old = vec![pin(1), pin(2)];
    assert_eq!(pin_changes(&old, &old[..1]).0, vec![PinChange::Removed { pin: old[1].clone() }]);
    for (direction, kind) in [(PinDirection::Output, old[0].kind.clone()), (PinDirection::Input, PinKind::Exec), (PinDirection::Input, PinKind::Data(DataType::Bool))] {
        let mut new = old.clone();
        new[0].direction = direction;
        new[0].kind = kind;
        assert_eq!(pin_changes(&old, &new).0, vec![PinChange::Changed { old: old[0].clone(), new: new[0].clone() }]);
    }
    let mut required = old.clone();
    required[0].required = true;
    assert_eq!(pin_changes(&old, &required).0, vec![PinChange::RequiredAdded { pin: required[0].clone() }]);
    assert_eq!(pin_changes(&required, &old), (vec![], true));
}

#[test]
fn same_id_in_both_directions_is_compared_separately() {
    let mut output = pin(1);
    output.direction = PinDirection::Output;
    let old = vec![pin(1), output.clone()];
    assert_eq!(pin_changes(&old, &[output]).0, vec![PinChange::Changed { old: pin(1), new: old[1].clone() }]);
}
