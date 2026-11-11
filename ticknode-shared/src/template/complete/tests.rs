use super::*;
use crate::template::environment;
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

#[test]
fn test_basic_variable_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![("value".to_string(), DataType::Number)];
    let (source, res) = complete_at("Hi {{ va|^| }}", &inputs, &types);
    let c = res.expect("should find completion in expression");
    assert_eq!(&source[c.range.clone()], "va");
    let item = c.items.iter().find(|i| i.label == "value").expect("must offer value");
    assert_eq!(item.kind, CompletionKind::Variable);
    assert_eq!(item.detail, "Number");
    assert_eq!(item.insert, "value");
}

#[test]
fn test_outside_and_comments_and_strings_return_none() {
    let types = TypeRegistry::new();
    let inputs = vec![("value".to_string(), DataType::Number)];

    let (_, res1) = complete_at("Hi |^|", &inputs, &types);
    assert!(res1.is_none(), "outside expression must return None");

    let (_, res2) = complete_at("{# va|^| #}", &inputs, &types);
    assert!(res2.is_none(), "inside comment must return None");

    let (_, res3) = complete_at("{{ 'va|^|' }}", &inputs, &types);
    assert!(res3.is_none(), "inside single-quoted string must return None");

    let (_, res4) = complete_at("{{ \"va|^|\" }}", &inputs, &types);
    assert!(res4.is_none(), "inside double-quoted string must return None");
}

#[test]
fn test_unclosed_expression() {
    let types = TypeRegistry::new();
    let inputs = vec![("user".to_string(), DataType::String)];
    let (source, res) = complete_at("{{ us|^|", &inputs, &types);
    let c = res.expect("unclosed expression must be recognized");
    assert_eq!(&source[c.range], "us");
    assert!(c.items.iter().any(|i| i.label == "user"));
}

#[test]
fn test_struct_field_completion() {
    let user_struct = TypeDefinition::new_struct(
        "test.user",
        "User",
        vec![
            FieldDefinition::new("name", "Name", DataType::String),
            FieldDefinition::new("id", "ID", DataType::Number),
        ],
    );
    let types = TypeRegistry::from_definitions([user_struct]);
    let inputs = vec![("u".to_string(), DataType::Named("test.user".to_string()))];

    let (_, res_all) = complete_at("{{ u.|^| }}", &inputs, &types);
    let c_all = res_all.expect("should complete fields");
    let labels: Vec<_> = c_all.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, vec!["id", "name"]);
    assert_eq!(c_all.items[0].detail, "Number");
    assert_eq!(c_all.items[1].detail, "String");

    let (source, res_prefix) = complete_at("{{ u.na|^| }}", &inputs, &types);
    let c_prefix = res_prefix.expect("should complete na prefix");
    assert_eq!(&source[c_prefix.range], "na");
    assert_eq!(c_prefix.items.len(), 1);
    assert_eq!(c_prefix.items[0].label, "name");
}

#[test]
fn test_nested_struct_completion() {
    let address_struct = TypeDefinition::new_struct(
        "test.address",
        "Address",
        vec![
            FieldDefinition::new("city", "City", DataType::String),
            FieldDefinition::new("zip", "Zip Code", DataType::Number),
        ],
    );
    let user_struct = TypeDefinition::new_struct(
        "test.user",
        "User",
        vec![
            FieldDefinition::new("name", "Name", DataType::String),
            FieldDefinition::new("address", "Address", DataType::Named("test.address".to_string())),
        ],
    );
    let types = TypeRegistry::from_definitions([address_struct, user_struct]);
    let inputs = vec![("u".to_string(), DataType::Named("test.user".to_string()))];

    let (_, res) = complete_at("{{ u.address.|^| }}", &inputs, &types);
    let c = res.expect("nested struct fields should complete");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, vec!["city", "zip"]);

    let (_, res_end) = complete_at("{{ u.address.city.|^| }}", &inputs, &types);
    let c_end = res_end.expect("completed primitive path returns empty items");
    assert!(c_end.items.is_empty());
}

#[test]
fn test_text_root_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![
        ("voltage".to_string(), DataType::Number),
        ("label".to_string(), DataType::String),
    ];
    let (_, res) = complete_at("{{ text.|^| }}", &inputs, &types);
    let c = res.expect("text. should offer input names");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, vec!["label", "voltage"]);
    assert_eq!(c.items[0].detail, "String");
    assert_eq!(c.items[1].detail, "Number");
}

#[test]
fn test_filter_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![("x".to_string(), DataType::String)];

    let (_, res1) = complete_at("{{ x | up|^| }}", &inputs, &types);
    let c1 = res1.expect("pipe should trigger filter completion");
    assert_eq!(c1.items[0].label, "upper");
    assert_eq!(c1.items[0].kind, CompletionKind::Filter);

    // No space right after pipe: {{ x||^| }} has pipe then caret
    let (_, res2) = complete_at("{{ x||^| }}", &inputs, &types);
    let c2 = res2.expect("no-space pipe position should complete filters");
    assert_eq!(c2.items[0].label, "text");
    assert!(c2.items.iter().any(|i| i.label == "abs"));

    // No space right after pipe with prefix: {{ x|up|^| }}
    let (source3, res3) = complete_at("{{ x|up|^| }}", &inputs, &types);
    let c3 = res3.expect("filter with prefix without space");
    assert_eq!(&source3[c3.range], "up");
    assert_eq!(c3.items[0].label, "upper");
}

#[test]
fn test_test_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![("x".to_string(), DataType::Number)];

    let (_, res1) = complete_at("{{ x is def|^| }}", &inputs, &types);
    let c1 = res1.expect("is should trigger test completion");
    assert_eq!(c1.items[0].label, "defined");
    assert_eq!(c1.items[0].kind, CompletionKind::Test);

    let (_, res2) = complete_at("{{ x is not |^| }}", &inputs, &types);
    let c2 = res2.expect("is not should trigger test completion");
    assert!(c2.items.iter().any(|i| i.label == "defined"));
    assert!(c2.items.iter().any(|i| i.label == "none"));
}

#[test]
fn test_tag_keyword_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![];

    let (_, res1) = complete_at("{% |^|", &inputs, &types);
    let c1 = res1.expect("first word of tag offers keywords");
    assert!(c1.items.iter().any(|i| i.label == "for" && i.kind == CompletionKind::Keyword));

    let (source2, res2) = complete_at("{%- fo|^| %}", &inputs, &types);
    let c2 = res2.expect("tag keyword with prefix");
    assert_eq!(&source2[c2.range], "fo");
    assert_eq!(c2.items.len(), 1);
    assert_eq!(c2.items[0].label, "for");
}

#[test]
fn test_for_loop_and_loop_attributes() {
    let user_struct = TypeDefinition::new_struct(
        "test.user",
        "User",
        vec![
            FieldDefinition::new("name", "Name", DataType::String),
            FieldDefinition::new("id", "ID", DataType::Number),
        ],
    );
    let types = TypeRegistry::from_definitions([user_struct]);
    let inputs = vec![(
        "items".to_string(),
        DataType::List(Box::new(DataType::Named("test.user".to_string()))),
    )];

    // item. inside for loop offers fields of test.user
    let (_, res1) = complete_at(
        "{% for item in items %}{{ item.|^| }}",
        &inputs,
        &types,
    );
    let c1 = res1.expect("item fields should be offered");
    let labels1: Vec<_> = c1.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels1, vec!["id", "name"]);

    // loop. inside for loop offers loop attributes
    let (_, res2) = complete_at(
        "{% for item in items %}{{ loop.|^| }}",
        &inputs,
        &types,
    );
    let c2 = res2.expect("loop. should offer loop attributes");
    assert!(c2.items.iter().any(|i| i.label == "index" && i.kind == CompletionKind::Attribute));
    assert!(c2.items.iter().any(|i| i.label == "first"));

    // after {% endfor %}: neither item nor loop offered
    let (_, res3) = complete_at(
        "{% for item in items %}{% endfor %}{{ item.|^| }}",
        &inputs,
        &types,
    );
    let c3 = res3.expect("closed loop results in empty fields");
    assert!(c3.items.is_empty());

    let (_, res4) = complete_at(
        "{% for item in items %}{% endfor %}{{ |^| }}",
        &inputs,
        &types,
    );
    let c4 = res4.expect("general expression after endfor");
    assert!(!c4.items.iter().any(|i| i.label == "item"));
    assert!(!c4.items.iter().any(|i| i.label == "loop"));
}

#[test]
fn test_set_variable_completion() {
    let types = TypeRegistry::new();
    let inputs = vec![];
    let (source, res) = complete_at("{% set total = 1 %}{{ to|^| }}", &inputs, &types);
    let c = res.expect("assigned set variable should be offered");
    assert_eq!(&source[c.range], "to");
    assert!(c.items.iter().any(|i| i.label == "total" && i.kind == CompletionKind::Variable));
}

#[test]
fn test_caret_in_middle_of_word() {
    let types = TypeRegistry::new();
    let inputs = vec![("value".to_string(), DataType::Number)];
    let (source, res) = complete_at("{{ val|^|ue }}", &inputs, &types);
    let c = res.expect("word in middle should complete");
    assert_eq!(&source[c.range], "value");
    assert!(c.items.iter().any(|i| i.label == "value"));
}

#[test]
fn test_multibyte_prefix() {
    let types = TypeRegistry::new();
    let inputs = vec![("value".to_string(), DataType::Number)];
    let (source, res) = complete_at("Привіт {{ va|^| }}", &inputs, &types);
    let c = res.expect("multibyte prefix preserves byte offset");
    assert_eq!(&source[c.range], "va");
    assert!(c.items.iter().any(|i| i.label == "value"));
}

#[test]
fn test_every_filter_and_test_exists_in_minijinja() {
    let env = environment();

    // Verify all declared built-in filters compile in MiniJinja
    for &(name, _) in catalog::BUILTIN_FILTERS {
        let template_src = format!("{{{{ 1 | {name} }}}}");
        let result = env.render_str(&template_src, ());
        if let Err(err) = result {
            let msg = err.to_string();
            assert!(
                !msg.contains("unknown filter"),
                "Filter '{name}' reported as unknown filter: {msg}"
            );
        }
    }

    // Verify all declared tests compile in MiniJinja
    for &name in catalog::TESTS {
        let template_src = format!("{{{{ 1 is {name} }}}}");
        let result = env.render_str(&template_src, ());
        if let Err(err) = result {
            let msg = err.to_string();
            assert!(
                !msg.contains("unknown test"),
                "Test '{name}' reported as unknown test: {msg}"
            );
        }
    }

    // Verify globals / functions
    for &(name, _) in catalog::FUNCTIONS {
        assert!(
            env.globals().any(|(global, _)| global == name),
            "Global function '{name}' missing from environment"
        );
    }
}

#[test]
fn unparsable_for_header_keeps_endfor_balanced() {
    let types = TypeRegistry::new();
    let inputs = vec![("items".to_string(), DataType::List(Box::new(DataType::Number)))];
    let source = "{% for a in items %}{% for in %}{% endfor %}{{ ";
    let result = complete(&CompletionInput { source, caret: source.len(), inputs: &inputs, types: &types }).unwrap();
    assert!(result.items.iter().any(|i| i.label == "a"));
    assert!(result.items.iter().any(|i| i.label == "loop"));
}
