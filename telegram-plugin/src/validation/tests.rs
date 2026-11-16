use super::*;

#[test]
fn properties_validate_keyboard_ranges_text_limits_ids_and_choices() {
    let props = Properties::new();
    for (name, valid, invalid) in [
        ("count", "24", "25"),
        ("columns", "8", "9"),
        ("resize", "yes", "maybe"),
        ("one_time", "no", "true"),
        ("persistent", "yes", "false"),
        ("action", "location", "unknown"),
        ("kind", "reply", "auto"),
        ("parse_mode", "markdown_v2", "Markdown"),
        ("text_mode", "regex", "glob"),
        ("data_mode", "prefix", "suffix"),
        ("chat", "-123", "abc"),
        ("user", "42", "1.5"),
        ("topic", "", "9223372036854775808"),
        ("command", "/status_1", "UPPER"),
    ] {
        assert!(property("keyboard", name, valid, &props).is_ok(), "{name}");
        assert!(
            property("keyboard", name, invalid, &props).is_err(),
            "{name}"
        );
    }
    for name in ["count", "columns"] {
        for invalid in ["0", "-1", "bad", "4294967296"] {
            assert!(property("keyboard", name, invalid, &props).is_err());
        }
        assert!(property("keyboard", name, "1", &props).is_ok());
    }
    assert!(property("download", "kind", "auto", &props).is_ok());
    assert!(property("download", "kind", "photo", &props).is_ok());
    assert!(property("download", "kind", "inline", &props).is_err());
    for (name, limit) in [("placeholder", 64), ("toast", 200)] {
        assert!(property("keyboard", name, &"я".repeat(limit), &props).is_ok());
        assert!(property("keyboard", name, &"я".repeat(limit + 1), &props).is_err());
    }
    assert!(node_property("keyboard", "count", &1i64.into(), &props).is_err());
}

#[test]
fn message_regex_validation_uses_both_candidate_and_stored_properties() {
    let mut props = Properties::from([("text_mode".into(), "regex".into())]);
    assert!(property("message", "pattern", "[", &props).is_err());
    assert!(property("message", "pattern", "^valid$", &props).is_ok());
    props.insert("pattern".into(), "[".into());
    assert!(property("message", "text_mode", "regex", &props).is_err());
    assert!(property("message", "text_mode", "equals", &props).is_ok());
}
