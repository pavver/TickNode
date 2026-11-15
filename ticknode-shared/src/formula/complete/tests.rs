use super::*;

#[test]
fn test_complete_empty_prefix_returns_all() {
    let inputs = vec![("temp".to_string(), DataType::Number)];
    let res = complete("", 0, &inputs);

    assert_eq!(res.range, 0..0);
    assert!(
        res.items
            .iter()
            .any(|i| i.label == "temp" && i.kind == CompletionKind::Variable)
    );
    assert!(
        res.items
            .iter()
            .any(|i| i.label == "pi" && i.kind == CompletionKind::Constant)
    );
    assert!(
        res.items
            .iter()
            .any(|i| i.label == "sqrt" && i.kind == CompletionKind::Function)
    );
}

#[test]
fn test_complete_prefix_filtering() {
    let inputs = vec![("sensor_temp".to_string(), DataType::Number)];
    let source = "sen";
    let res = complete(source, 3, &inputs);

    assert_eq!(res.range, 0..3);
    assert_eq!(res.items.len(), 1);
    assert_eq!(res.items[0].label, "sensor_temp");
}

#[test]
fn test_complete_function_signature_and_doc_key() {
    let source = "cl";
    let res = complete(source, 2, &[]);

    let clamp_item = res.items.iter().find(|i| i.label == "clamp");
    assert!(clamp_item.is_some());
    let item = clamp_item.unwrap();
    assert_eq!(item.kind, CompletionKind::Function);
    assert_eq!(item.detail, "clamp(x, lo, hi)");
    assert_eq!(item.insert, "clamp(");
    assert_eq!(item.caret, Some(6));
    assert_eq!(
        item.help.as_ref().map(|m| m.key.as_str()),
        Some("formula-fn-clamp")
    );
}

#[test]
fn test_complete_constant_doc_key() {
    let source = "pi";
    let res = complete(source, 2, &[]);

    let pi_item = res.items.iter().find(|i| i.label == "pi");
    assert!(pi_item.is_some());
    let item = pi_item.unwrap();
    assert_eq!(item.kind, CompletionKind::Constant);
    assert_eq!(
        item.help.as_ref().map(|m| m.key.as_str()),
        Some("formula-const-pi")
    );
}
