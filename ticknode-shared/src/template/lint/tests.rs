use super::*;
use crate::types::{FieldDefinition, TypeDefinition};

fn setup_test_types() -> (TypeRegistry, Vec<(String, DataType)>) {
    let types = TypeRegistry::from_definitions([
        TypeDefinition::new_struct(
            "Address",
            "Address",
            vec![
                FieldDefinition::new("street", "Street", DataType::String),
                FieldDefinition::new("city", "City", DataType::String),
            ],
        ),
        TypeDefinition::new_struct(
            "User",
            "User Profile",
            vec![
                FieldDefinition::new("name", "Name", DataType::String),
                FieldDefinition::new("age", "Age", DataType::Number),
                FieldDefinition::new("address", "Address", DataType::Named("Address".to_string())),
            ],
        ),
    ]);

    let inputs = vec![
        ("user".to_string(), DataType::Named("User".to_string())),
        ("num".to_string(), DataType::Number),
        ("str_val".to_string(), DataType::String),
        ("any_val".to_string(), DataType::Any),
    ];

    (types, inputs)
}

#[test]
fn test_unknown_filter_without_and_with_suggestion() {
    let (types, inputs) = setup_test_types();

    // Unknown filter without suggestion (edit distance > 2)
    let lints = lint("{{ str_val | non_existent_filter_xyz }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-filter");
    assert_eq!(lints[0].range, 13..36);
    assert_eq!(
        lints[0].message.args,
        vec![("name".to_string(), "non_existent_filter_xyz".to_string())]
    );

    // Unknown filter with suggestion: 'abss' -> 'abs' (dist 1)
    let lints = lint("{{ num | abss }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-filter-suggest");
    assert_eq!(lints[0].range, 9..13);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("name".to_string(), "abss".to_string()),
            ("suggestion".to_string(), "abs".to_string()),
        ]
    );

    // Unknown filter with suggestion: 'formt' -> 'format' (dist 1)
    let lints = lint("{{ str_val | formt }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-filter-suggest");
    assert_eq!(lints[0].range, 13..18);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("name".to_string(), "formt".to_string()),
            ("suggestion".to_string(), "format".to_string()),
        ]
    );

    // Unknown filter with suggestion: 'txt' -> 'text' (dist 1)
    let lints = lint("{{ num | txt }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-filter-suggest");
    assert_eq!(
        lints[0].message.args,
        vec![
            ("name".to_string(), "txt".to_string()),
            ("suggestion".to_string(), "text".to_string()),
        ]
    );
}

#[test]
fn test_filter_type_mismatch_and_match() {
    let (types, inputs) = setup_test_types();

    // Incompatible: 'user.name' is String, 'abs' expects Number
    let lints = lint("{{ user.name | abs }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-filter-type");
    assert_eq!(lints[0].range, 15..18);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("name".to_string(), "abs".to_string()),
            ("type".to_string(), "String".to_string()),
        ]
    );

    // Compatible: 'user.age' is Number, 'abs' expects Number -> no lint
    let lints = lint("{{ user.age | abs }}", &inputs, &types);
    assert!(lints.is_empty());

    // Compatible: 'user.name' is String, 'upper' expects String -> no lint
    let lints = lint("{{ user.name | upper }}", &inputs, &types);
    assert!(lints.is_empty());
}

#[test]
fn test_unknown_struct_fields_and_suggestions() {
    let (types, inputs) = setup_test_types();

    // Unknown field without suggestion (edit distance > 2)
    let lints = lint("{{ user.completely_unknown_field }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-field");
    assert_eq!(lints[0].range, 8..32);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("field".to_string(), "completely_unknown_field".to_string()),
            ("type".to_string(), "User Profile".to_string()),
        ]
    );

    // Unknown field with suggestion: 'ag' -> 'age' (dist 1)
    let lints = lint("{{ user.ag }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-field-suggest");
    assert_eq!(lints[0].range, 8..10);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("field".to_string(), "ag".to_string()),
            ("type".to_string(), "User Profile".to_string()),
            ("suggestion".to_string(), "age".to_string()),
        ]
    );

    // Nested struct unknown field with suggestion: 'streeet' -> 'street' (dist 1)
    let lints = lint("{{ user.address.streeet }}", &inputs, &types);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-field-suggest");
    assert_eq!(lints[0].range, 16..23);
    assert_eq!(
        lints[0].message.args,
        vec![
            ("field".to_string(), "streeet".to_string()),
            ("type".to_string(), "Address".to_string()),
            ("suggestion".to_string(), "street".to_string()),
        ]
    );

    // Valid fields -> no lints
    let lints = lint("{{ user.address.street }}", &inputs, &types);
    assert!(lints.is_empty());
}

#[test]
fn test_unknown_test_after_is_and_is_not() {
    let (types, inputs) = setup_test_types();

    // Unknown test after 'is'
    let lints = lint(
        "{% if num is not_a_real_test %}ok{% endif %}",
        &inputs,
        &types,
    );
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-test");
    assert_eq!(lints[0].range, 13..28);
    assert_eq!(
        lints[0].message.args,
        vec![("name".to_string(), "not_a_real_test".to_string())]
    );

    // Unknown test after 'is not'
    let lints = lint(
        "{% if num is not bogus_test %}ok{% endif %}",
        &inputs,
        &types,
    );
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "template-lint-unknown-test");
    assert_eq!(lints[0].range, 17..27);
    assert_eq!(
        lints[0].message.args,
        vec![("name".to_string(), "bogus_test".to_string())]
    );

    // Known tests -> no lints
    let lints = lint(
        "{% if num is defined and num is not none and num is even %}ok{% endif %}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());
}

#[test]
fn test_no_false_positives_for_locals_loop_text_any_and_default() {
    let (types, inputs) = setup_test_types();

    // 1. Local variable in 'for' loop: item.unknown and item | abs should not lint
    let lints = lint(
        "{% for item in [1, 2] %}{{ item.field_unknown }} {{ item | abs }}{% endfor %}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());

    // 2. Local variable in 'set': x.field and x | abs should not lint
    let lints = lint(
        "{% set x = 123 %}{{ x.unknown_prop }} {{ x | abs }}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());

    // 3. loop.* should not lint
    let lints = lint(
        "{% for a in [1] %}{{ loop.index }} {{ loop.random_attr }}{% endfor %}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());

    // 4. text.* should not lint
    let lints = lint(
        "{{ text.power }} {{ text.undeclared_var }}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());

    // 5. Any-typed input should not lint on filter or fields
    let lints = lint(
        "{{ any_val.field_xyz }} {{ any_val | abs }}",
        &inputs,
        &types,
    );
    assert!(lints.is_empty());

    // 6. Filter chained after 'default' preserves inferred type
    let lints = lint("{{ num | default(0) | abs }}", &inputs, &types);
    assert!(lints.is_empty());
}

#[test]
fn test_syntax_errors_return_no_lints() {
    let (types, inputs) = setup_test_types();

    // Unclosed bracket is a syntax error; lint must return empty
    let lints = lint("{{ user.ag", &inputs, &types);
    assert!(lints.is_empty());
}
