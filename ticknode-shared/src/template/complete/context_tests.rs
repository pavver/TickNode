use super::*;
use crate::types::{FieldDefinition, TypeDefinition};

fn parse_caret(template: &str) -> (String, usize) {
    let caret_marker = "|^|";
    let pos = template
        .find(caret_marker)
        .expect("test template must contain caret marker |^|");
    let source = template.replace(caret_marker, "");
    (source, pos)
}

fn complete_at<'a>(
    template: &'a str,
    inputs: &'a [(String, DataType)],
    types: &'a TypeRegistry,
) -> (String, Option<Completion>) {
    let (source, caret) = parse_caret(template);
    let input = CompletionInput {
        source: &source,
        caret,
        inputs,
        types,
    };
    let completion = complete(&input);
    (source, completion)
}

fn setup_test_context() -> (Vec<(String, DataType)>, TypeRegistry) {
    let user_struct = TypeDefinition::new_struct(
        "test.user",
        "User",
        vec![
            FieldDefinition::new("name", "Name", DataType::String),
            FieldDefinition::new("id", "ID", DataType::Number),
        ],
    );
    let mut defs = vec![user_struct];
    defs.extend(crate::quantity::catalog().iter().cloned());
    let types = TypeRegistry::from_definitions(defs);

    let inputs = vec![
        ("name".to_string(), DataType::String),
        ("power".to_string(), DataType::Number),
        ("on".to_string(), DataType::Bool),
        (
            "items".to_string(),
            DataType::List(Box::new(DataType::Named("test.user".to_string()))),
        ),
        ("u".to_string(), DataType::Named("test.user".to_string())),
        ("any".to_string(), DataType::Any),
        (
            "q_power".to_string(),
            DataType::Named("quantity.power".to_string()),
        ),
    ];

    (inputs, types)
}

#[test]
fn test_string_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ name | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for name");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["upper", "trim", "length", "text"] {
        assert!(labels.contains(&expected), "must contain {expected}");
    }
    for forbidden in ["abs", "round", "join", "sum", "items"] {
        assert!(!labels.contains(&forbidden), "must not contain {forbidden}");
    }
}

#[test]
fn test_number_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ power | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for power");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels[0], "text", "text must be first for Number input");
    assert!(labels.contains(&"round"));
    assert!(labels.contains(&"abs"));
    for forbidden in ["upper", "join", "split"] {
        assert!(!labels.contains(&forbidden), "must not contain {forbidden}");
    }
}

#[test]
fn test_list_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ items | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for items");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["join", "first", "length", "selectattr", "map"] {
        assert!(labels.contains(&expected), "must contain {expected}");
    }
    for forbidden in ["upper", "round"] {
        assert!(!labels.contains(&forbidden), "must not contain {forbidden}");
    }
}

#[test]
fn test_chained_filter_list_to_map() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ items | first | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for items | first");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["items", "dictsort", "attr"] {
        assert!(labels.contains(&expected), "must contain {expected}");
    }
    assert!(!labels.contains(&"upper"), "must not contain upper");
}

#[test]
fn test_chained_filter_string_to_number() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ name | length | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for name | length");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels[0], "text", "text must be first for Number output");
    assert!(labels.contains(&"abs"));
    assert!(labels.contains(&"round"));
    assert!(!labels.contains(&"upper"));
}

#[test]
fn test_filter_with_parentheses_and_strings() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ name | replace('a', ')') | |^|}}", &inputs, &types);
    let c = res.expect("should complete filters for replace with paren string");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"upper"));
    assert!(labels.contains(&"trim"));
    assert!(!labels.contains(&"abs"));
}

#[test]
fn test_dotted_paths_and_special_contexts() {
    let (inputs, types) = setup_test_context();
    // u.id -> Number
    let (_, res_id) = complete_at("{{ u.id | |^|}}", &inputs, &types);
    let c_id = res_id.expect("u.id completion");
    let labels_id: Vec<_> = c_id.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels_id.contains(&"round"));
    assert!(!labels_id.contains(&"upper"));

    // u -> Map
    let (_, res_u) = complete_at("{{ u | |^|}}", &inputs, &types);
    let c_u = res_u.expect("u completion");
    let labels_u: Vec<_> = c_u.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels_u.contains(&"items"));
    assert!(!labels_u.contains(&"upper"));

    // text.power -> String
    let (_, res_text) = complete_at("{{ text.power | |^|}}", &inputs, &types);
    let c_text = res_text.expect("text.power completion");
    let labels_text: Vec<_> = c_text.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels_text.contains(&"upper"));
    assert!(!labels_text.contains(&"abs"));

    // loop.index inside for -> Number
    let (_, res_loop) = complete_at(
        "{% for x in items %}{{ loop.index | |^|}}{% endfor %}",
        &inputs,
        &types,
    );
    let c_loop = res_loop.expect("loop.index completion");
    let labels_loop: Vec<_> = c_loop.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels_loop.contains(&"round"));
    assert!(!labels_loop.contains(&"upper"));

    // quantity.power -> Number
    let (_, res_q) = complete_at("{{ q_power | |^|}}", &inputs, &types);
    let c_q = res_q.expect("quantity.power completion");
    let labels_q: Vec<_> = c_q.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels_q.contains(&"round"));
    assert!(!labels_q.contains(&"upper"));
}

#[test]
fn test_any_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ any | |^|}}", &inputs, &types);
    let c = res.expect("should complete all filters for any");
    assert_eq!(c.items[0].label, "text", "text must be first for Any input");
    for &(name, _) in catalog::BUILTIN_FILTERS {
        assert!(
            c.items.iter().any(|i| i.label == name),
            "all filters must be present for any: {name}"
        );
    }
}

#[test]
fn test_type_aware_test_completion() {
    let (inputs, types) = setup_test_context();
    // power is -> odd, even, divisibleby, defined; not startingwith, mapping
    let (_, res_p) = complete_at("{{ power is |^|}}", &inputs, &types);
    let c_p = res_p.expect("power is completion");
    let labels_p: Vec<_> = c_p.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["odd", "even", "divisibleby", "defined"] {
        assert!(
            labels_p.contains(&expected),
            "power is must contain {expected}"
        );
    }
    for forbidden in ["startingwith", "mapping"] {
        assert!(
            !labels_p.contains(&forbidden),
            "power is must not contain {forbidden}"
        );
    }

    // name is -> startingwith, lower; not odd
    let (_, res_n) = complete_at("{{ name is |^|}}", &inputs, &types);
    let c_n = res_n.expect("name is completion");
    let labels_n: Vec<_> = c_n.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["startingwith", "lower"] {
        assert!(
            labels_n.contains(&expected),
            "name is must contain {expected}"
        );
    }
    assert!(!labels_n.contains(&"odd"), "name is must not contain odd");
}

#[test]
fn test_for_in_iterable_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{% for x in |^|", &inputs, &types);
    let c = res.expect("for in completion");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["items", "name", "any", "range"] {
        assert!(
            labels.contains(&expected),
            "must contain iterable {expected}"
        );
    }
    for forbidden in ["power", "on"] {
        assert!(
            !labels.contains(&forbidden),
            "must not contain non-iterable {forbidden}"
        );
    }
}

#[test]
fn test_filter_detail_with_output_kind() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ name | len|^|}}", &inputs, &types);
    let c = res.expect("length completion");
    let item = c
        .items
        .iter()
        .find(|i| i.label == "length")
        .expect("must offer length");
    assert!(
        item.detail.contains("→ Number"),
        "detail must contain → Number, got: {}",
        item.detail
    );
}
