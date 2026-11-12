use super::*;
use ticknode_shared::{FieldDefinition, PinDefinition, TypeDefinition};

fn node(name: &str, pins: Vec<(u32, PinDirection, PinType)>) -> NodeDefinition {
    NodeDefinition {
        adaptive: false,
        type_name: name.into(),
        title: name.into(),
        category: "Data".into(),
        description: String::new(),
        help: String::new(),
        properties: vec![],
        cost: 1,
        pins: pins
            .into_iter()
            .map(|(id, direction, pin_type)| PinDefinition {
                id: PinId(id),
                name: String::new(),
                direction,
                pin_type,
                required: false,
            })
            .collect(),
    }
}

fn find(
    direction: PinDirection,
    kind: PinType,
    catalog: &[NodeDefinition],
    types: &TypeRegistry,
) -> Vec<Suggestion> {
    suggestions(direction, &kind, catalog, types, |d| {
        (d.title.clone(), d.category.clone())
    })
}

#[test]
fn numbers_rank_math_before_any_and_sort_translated_titles() {
    use PinDirection::Input;
    let catalog = [
        node("Any sink", vec![(0, Input, PinType::Data(DataType::Any))]),
        node(
            "math/sub",
            vec![(1, Input, PinType::Data(DataType::Number))],
        ),
        node(
            "math/add",
            vec![(2, Input, PinType::Data(DataType::Number))],
        ),
    ];
    let result = find(
        PinDirection::Output,
        PinType::Data(DataType::Number),
        &catalog,
        &TypeRegistry::new(),
    );
    assert_eq!(
        result
            .iter()
            .map(|s| (s.type_name.as_str(), s.rank))
            .collect::<Vec<_>>(),
        [("math/add", 0), ("math/sub", 0), ("Any sink", 3)]
    );
    let translated = suggestions(
        PinDirection::Output,
        &PinType::Data(DataType::Number),
        &catalog,
        &TypeRegistry::new(),
        |d| {
            (
                if d.type_name == "math/sub" {
                    "A".into()
                } else {
                    "Z".into()
                },
                d.category.clone(),
            )
        },
    );
    assert_eq!(translated[0].type_name, "math/sub");
}

#[test]
fn exec_only_matches_opposite_exec_pins() {
    let catalog = [
        node("exec", vec![(1, PinDirection::Input, PinType::Exec)]),
        node(
            "data",
            vec![(0, PinDirection::Input, PinType::Data(DataType::Any))],
        ),
        node("output", vec![(0, PinDirection::Output, PinType::Exec)]),
    ];
    let result = find(
        PinDirection::Output,
        PinType::Exec,
        &catalog,
        &TypeRegistry::new(),
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].type_name, "exec");
}

#[test]
fn input_drag_uses_outputs_and_skips_conversion_only_pins() {
    let catalog = [
        node(
            "source",
            vec![(4, PinDirection::Output, PinType::Data(DataType::Number))],
        ),
        node(
            "text source",
            vec![(2, PinDirection::Output, PinType::Data(DataType::String))],
        ),
        node(
            "sink",
            vec![(1, PinDirection::Input, PinType::Data(DataType::Number))],
        ),
    ];
    let result = find(
        PinDirection::Input,
        PinType::Data(DataType::Number),
        &catalog,
        &TypeRegistry::new(),
    );
    assert_eq!(result.len(), 1);
    assert_eq!(
        (result[0].type_name.as_str(), result[0].pin, result[0].rank),
        ("source", PinId(4), 0)
    );
    assert!(
        find(
            PinDirection::Output,
            PinType::Data(DataType::String),
            &catalog,
            &TypeRegistry::new()
        )
        .is_empty()
    );
}

#[test]
fn bool_output_skips_number_inputs_including_adaptive_math() {
    use PinDirection::{Input, Output};
    let mut math = node(
        "math/add",
        vec![
            (1, Input, PinType::Data(DataType::Number)),
            (0, Output, PinType::Data(DataType::Number)),
        ],
    );
    math.adaptive = true;
    let mut text = node(
        "text/to_text",
        vec![(1, Input, PinType::Data(DataType::Any))],
    );
    text.adaptive = true;
    let catalog = [
        math,
        text,
        node(
            "for_loop",
            vec![(1, Input, PinType::Data(DataType::Number))],
        ),
        node("logic/and", vec![(1, Input, PinType::Data(DataType::Bool))]),
    ];
    let result = find(
        Output,
        PinType::Data(DataType::Bool),
        &catalog,
        &TypeRegistry::new(),
    );
    let names: Vec<_> = result
        .iter()
        .map(|s| (s.type_name.as_str(), s.rank))
        .collect();
    assert_eq!(names, [("logic/and", 0), ("text/to_text", 0)]);
}

#[test]
fn named_struct_prefers_its_break_over_object_and_rejects_reverse() {
    let types = TypeRegistry::from_definitions(vec![TypeDefinition::new_struct(
        "test.point",
        "Point",
        vec![FieldDefinition::new("x", "X", DataType::Number)],
    )]);
    let named = DataType::Named("test.point".into());
    let catalog = [
        node(
            "type/break/test.point",
            vec![(0, PinDirection::Input, PinType::Data(named.clone()))],
        ),
        node(
            "object",
            vec![(1, PinDirection::Input, PinType::Data(DataType::Object))],
        ),
    ];
    let result = find(
        PinDirection::Output,
        PinType::Data(named.clone()),
        &catalog,
        &types,
    );
    assert_eq!(
        (result[0].type_name.as_str(), result[0].rank),
        ("type/break/test.point", 0)
    );
    assert_eq!(result[1].rank, 1);
    let catalog = [node(
        "object",
        vec![(0, PinDirection::Output, PinType::Data(DataType::Object))],
    )];
    assert!(find(PinDirection::Input, PinType::Data(named), &catalog, &types).is_empty());
}

#[test]
fn best_rank_then_lowest_id_and_unavailable_are_respected() {
    use PinDirection::Input;
    let mut unavailable = node("stub", vec![(0, Input, PinType::Data(DataType::Number))]);
    unavailable.category = "Unavailable".into();
    let catalog = [
        node(
            "mixed",
            vec![
                (0, Input, PinType::Data(DataType::Any)),
                (1, Input, PinType::Data(DataType::String)),
                (9, Input, PinType::Data(DataType::Number)),
                (4, Input, PinType::Data(DataType::Number)),
            ],
        ),
        unavailable,
    ];
    let result = find(
        PinDirection::Output,
        PinType::Data(DataType::Number),
        &catalog,
        &TypeRegistry::new(),
    );
    assert_eq!(result.len(), 1);
    assert_eq!((result[0].pin, result[0].rank), (PinId(4), 0));
}

#[test]
fn adaptive_definitions_match_fitting_data_types_in_the_opposite_direction_only() {
    for direction in [PinDirection::Input, PinDirection::Output] {
        let opposite = if direction == PinDirection::Input {
            PinDirection::Output
        } else {
            PinDirection::Input
        };
        let mut adaptive = node(
            "adaptive",
            vec![
                (7, opposite, PinType::Data(DataType::Number)),
                (3, opposite, PinType::Data(DataType::Any)),
                (0, direction, PinType::Data(DataType::Duration)),
            ],
        );
        adaptive.adaptive = true;
        for data_type in [
            DataType::Duration,
            DataType::Named("test.point".into()),
            DataType::List(Box::new(DataType::Bool)),
            DataType::Any,
        ] {
            let result = find(
                direction,
                PinType::Data(data_type),
                &[adaptive.clone()],
                &TypeRegistry::new(),
            );
            assert_eq!(result.len(), 1);
            assert_eq!((result[0].rank, result[0].pin), (0, PinId(3)));
        }
        assert!(find(direction, PinType::Exec, &[adaptive], &TypeRegistry::new()).is_empty());
    }
}

fn suggestion(name: &str, title: &str, group: &str) -> Suggestion {
    Suggestion {
        type_name: name.into(),
        title: title.into(),
        category: group.into(),
        group: group.into(),
        pin: PinId(0),
        rank: 0,
        preference: usize::MAX,
    }
}

#[test]
fn arrange_groups_by_category_then_title_after_popular_types() {
    let list = vec![
        suggestion("b", "Beta", "Math"),
        suggestion("z", "Zeta", "Events"),
        suggestion("a", "Alpha", "Math"),
        suggestion("c", "Gamma", "Events"),
        suggestion("h", "Home", "Home Assistant"),
        suggestion("d", "Delta", "Math"),
    ];
    let usage = HashMap::from([("d".to_string(), 3), ("a".to_string(), 1)]);
    let recent = vec!["h".to_string(), "a".to_string()];
    let rank = |g: &str| match g {
        "Events" => 0,
        "Math" => 1,
        _ => 9,
    };
    let order: Vec<_> = arrange(list, &usage, &recent, rank)
        .into_iter()
        .map(|(p, s)| (p, s.type_name))
        .collect();
    let expect = [
        (true, "d"),
        (true, "a"),
        (true, "h"),
        (false, "c"),
        (false, "z"),
        (false, "b"),
    ];
    assert_eq!(order, expect.map(|(p, n)| (p, n.to_string())));
}

#[test]
fn arrange_limits_popular_section() {
    let list: Vec<_> = (0..8)
        .map(|i| suggestion(&format!("n{i}"), &format!("T{i}"), "Math"))
        .collect();
    let usage: HashMap<_, _> = (0..8).map(|i| (format!("n{i}"), 1)).collect();
    let result = arrange(list, &usage, &[], |_| 0);
    assert_eq!(result.iter().filter(|(p, _)| *p).count(), MAX_POPULAR);
    assert_eq!(result.len(), 8);
}

#[test]
fn text_template_comes_first_in_its_category_for_text_wires() {
    use PinDirection::{Input, Output};
    let mut template = node(
        "text/template",
        vec![
            (1, Input, PinType::Data(DataType::Any)),
            (0, Output, PinType::Data(DataType::String)),
        ],
    );
    template.adaptive = true;
    template.category = "String".into();
    let mut concat = node(
        "string/concat",
        vec![
            (1, Input, PinType::Data(DataType::String)),
            (0, Output, PinType::Data(DataType::String)),
        ],
    );
    concat.category = "String".into();
    let catalog = [concat, template];
    for direction in [Output, Input] {
        let list = suggestions(
            direction,
            &PinType::Data(DataType::String),
            &catalog,
            &TypeRegistry::new(),
            |d| {
                (
                    if d.type_name == "text/template" {
                        "Zz".into()
                    } else {
                        "Aa".into()
                    },
                    d.category.clone(),
                )
            },
        );
        let order: Vec<_> = arrange(list, &HashMap::new(), &[], |_| 0)
            .into_iter()
            .map(|(_, s)| s.type_name)
            .collect();
        assert_eq!(order, ["text/template", "string/concat"], "{direction:?}");
    }
    // Other data types keep title order.
    let list = suggestions(
        Output,
        &PinType::Data(DataType::Number),
        &catalog,
        &TypeRegistry::new(),
        |d| {
            (
                if d.type_name == "text/template" {
                    "Zz".into()
                } else {
                    "Aa".into()
                },
                d.category.clone(),
            )
        },
    );
    assert_eq!(
        arrange(list, &HashMap::new(), &[], |_| 0)[0].1.type_name,
        "text/template"
    );
}
