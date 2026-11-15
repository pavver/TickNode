use super::hover::find_formula_tokens;

#[test]
fn test_find_formula_tokens() {
    let source = "clamp(x + 10, 0, 100)";
    let tokens = find_formula_tokens(source);
    let names: Vec<&str> = tokens.iter().map(|r| &source[r.clone()]).collect();
    assert_eq!(names, vec!["clamp", "x", "10", "0", "100"]);
}
