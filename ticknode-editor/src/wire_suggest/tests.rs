use super::*;
use ticknode_shared::{FieldDefinition, PinDefinition, TypeDefinition};

fn node(name: &str, pins: Vec<(u32, PinDirection, PinType)>) -> NodeDefinition {
    NodeDefinition {
        adaptive: false,
        type_name: name.into(), title: name.into(), category: "Data".into(), description: String::new(),
        help: String::new(), properties: vec![], cost: 1,
        pins: pins.into_iter().map(|(id, direction, pin_type)| PinDefinition {
            id: PinId(id), name: String::new(), direction, pin_type, required: false,
        }).collect(),
    }
}

fn find(direction: PinDirection, kind: PinType, catalog: &[NodeDefinition], types: &TypeRegistry) -> Vec<Suggestion> {
    suggestions(direction, &kind, catalog, types, |d| (d.title.clone(), d.category.clone()))
}

#[test]
fn numbers_rank_math_before_any_and_sort_translated_titles() {
    use PinDirection::Input;
    let catalog = [node("Any sink", vec![(0, Input, PinType::Data(DataType::Any))]),
        node("math/sub", vec![(1, Input, PinType::Data(DataType::Number))]),
        node("math/add", vec![(2, Input, PinType::Data(DataType::Number))])];
    let result = find(PinDirection::Output, PinType::Data(DataType::Number), &catalog, &TypeRegistry::new());
    assert_eq!(result.iter().map(|s| (s.type_name.as_str(), s.rank)).collect::<Vec<_>>(),
        [("math/add", 0), ("math/sub", 0), ("Any sink", 3)]);
    let translated = suggestions(PinDirection::Output, &PinType::Data(DataType::Number), &catalog,
        &TypeRegistry::new(), |d| (if d.type_name == "math/sub" { "A".into() } else { "Z".into() }, d.category.clone()));
    assert_eq!(translated[0].type_name, "math/sub");
}

#[test]
fn exec_only_matches_opposite_exec_pins() {
    let catalog = [node("exec", vec![(1, PinDirection::Input, PinType::Exec)]),
        node("data", vec![(0, PinDirection::Input, PinType::Data(DataType::Any))]),
        node("output", vec![(0, PinDirection::Output, PinType::Exec)])];
    let result = find(PinDirection::Output, PinType::Exec, &catalog, &TypeRegistry::new());
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].type_name, "exec");
}

#[test]
fn input_drag_uses_outputs_and_conversion_direction() {
    let catalog = [node("source", vec![(4, PinDirection::Output, PinType::Data(DataType::String))]),
        node("sink", vec![(1, PinDirection::Input, PinType::Data(DataType::Number))])];
    let result = find(PinDirection::Input, PinType::Data(DataType::Number), &catalog, &TypeRegistry::new());
    assert_eq!(result.len(), 1);
    assert_eq!((result[0].pin, result[0].rank), (PinId(4), 2));
    let result = find(PinDirection::Output, PinType::Data(DataType::String), &catalog, &TypeRegistry::new());
    assert_eq!(result[0].rank, 2);
}

#[test]
fn named_struct_prefers_its_break_over_object_and_rejects_reverse() {
    let types = TypeRegistry::from_definitions(vec![TypeDefinition::new_struct("test.point", "Point",
        vec![FieldDefinition::new("x", "X", DataType::Number)])]);
    let named = DataType::Named("test.point".into());
    let catalog = [node("type/break/test.point", vec![(0, PinDirection::Input, PinType::Data(named.clone()))]),
        node("object", vec![(1, PinDirection::Input, PinType::Data(DataType::Object))])];
    let result = find(PinDirection::Output, PinType::Data(named.clone()), &catalog, &types);
    assert_eq!((result[0].type_name.as_str(), result[0].rank), ("type/break/test.point", 0));
    assert_eq!(result[1].rank, 1);
    let catalog = [node("object", vec![(0, PinDirection::Output, PinType::Data(DataType::Object))])];
    assert!(find(PinDirection::Input, PinType::Data(named), &catalog, &types).is_empty());
}

#[test]
fn best_rank_then_lowest_id_and_unavailable_are_respected() {
    use PinDirection::Input;
    let mut unavailable = node("stub", vec![(0, Input, PinType::Data(DataType::Number))]);
    unavailable.category = "Unavailable".into();
    let catalog = [node("mixed", vec![(0, Input, PinType::Data(DataType::Any)),
        (1, Input, PinType::Data(DataType::String)), (9, Input, PinType::Data(DataType::Number)),
        (4, Input, PinType::Data(DataType::Number))]), unavailable];
    let result = find(PinDirection::Output, PinType::Data(DataType::Number), &catalog, &TypeRegistry::new());
    assert_eq!(result.len(), 1);
    assert_eq!((result[0].pin, result[0].rank), (PinId(4), 0));
}

#[test]
fn adaptive_definitions_match_any_data_type_in_the_opposite_direction_only() {
    for direction in [PinDirection::Input, PinDirection::Output] {
        let opposite = if direction == PinDirection::Input { PinDirection::Output } else { PinDirection::Input };
        let mut adaptive = node("adaptive", vec![(7, opposite, PinType::Data(DataType::Number)),
            (3, opposite, PinType::Data(DataType::Any)), (0, direction, PinType::Data(DataType::Duration))]);
        adaptive.adaptive = true;
        for data_type in [DataType::Duration, DataType::Named("test.point".into()), DataType::List(Box::new(DataType::Bool)), DataType::Any] {
            let result = find(direction, PinType::Data(data_type), &[adaptive.clone()], &TypeRegistry::new());
            assert_eq!(result.len(), 1);
            assert_eq!((result[0].rank, result[0].pin), (0, PinId(3)));
        }
        assert!(find(direction, PinType::Exec, &[adaptive], &TypeRegistry::new()).is_empty());
    }
}
