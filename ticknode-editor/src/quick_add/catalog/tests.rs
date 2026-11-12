use super::*;

fn node(name: &str, title: &str, category: &str, description: &str) -> NodeDefinition {
    NodeDefinition {
        type_name: name.into(),
        title: title.into(),
        category: category.into(),
        description: description.into(),
        help: String::new(),
        pins: vec![],
        properties: vec![],
        adaptive: false,
        cost: 1,
    }
}

fn find(catalog: &[NodeDefinition], recent: &[String], query: &str) -> Vec<Choice> {
    ranked(catalog, recent, query, |d| {
        (d.title.clone(), d.category.clone(), d.description.clone())
    })
}

fn names(catalog: &[NodeDefinition], choices: &[Choice]) -> Vec<String> {
    choices
        .iter()
        .map(|c| catalog[c.index].type_name.clone())
        .collect()
}

#[test]
fn empty_query_promotes_available_recents_once_then_uses_palette_order() {
    let catalog = [
        node("z", "Z", "Data", ""),
        node("event", "Event", "Events", ""),
        node("a", "A", "Data", ""),
        node("unknown", "A", "Z custom", ""),
        node("unknown2", "Z", "A custom", ""),
    ];
    let result = find(
        &catalog,
        &["missing".into(), "z".into(), "z".into(), "a".into()],
        "   ",
    );
    assert_eq!(
        names(&catalog, &result),
        ["z", "a", "event", "unknown2", "unknown"]
    );
    assert!(result[..2].iter().all(|c| c.recent));
    assert!(result[2..].iter().all(|c| !c.recent));
}

#[test]
fn query_ranks_prefix_contains_and_other_fields_then_palette_order() {
    let catalog = [
        node("description", "A", "Events", "send a signal"),
        node("contains", "Resend", "Events", ""),
        node("prefix-data", "Send B", "Data", ""),
        node("prefix-event", "Send Z", "Events", ""),
        node("send/type", "B", "Data", ""),
        node("category", "C", "Send tools", ""),
    ];
    let result = find(&catalog, &["description".into()], "sEnD");
    assert_eq!(
        names(&catalog, &result),
        [
            "prefix-event",
            "prefix-data",
            "contains",
            "description",
            "send/type",
            "category"
        ]
    );
    assert!(result.iter().all(|c| !c.recent));
}

#[test]
fn unavailable_is_excluded_even_when_recent_or_searching_its_name() {
    let catalog = [
        node("missing", "Missing", "Unavailable", ""),
        node("available", "Available", "Data", ""),
    ];
    assert_eq!(
        names(&catalog, &find(&catalog, &["missing".into()], "")),
        ["available"]
    );
    assert!(find(&catalog, &[], "missing").is_empty());
}

#[test]
fn search_uses_translated_labels_and_all_terms_across_fields() {
    let catalog = [
        node("timer", "Timer", "Time", "tick"),
        node("other", "Other", "Data", ""),
    ];
    let choices = ranked(&catalog, &[], "ЧАС  очікування", |d| {
        if d.type_name == "timer" {
            ("Таймер".into(), "Час".into(), "Час очікування".into())
        } else {
            (d.title.clone(), d.category.clone(), d.description.clone())
        }
    });
    assert_eq!(names(&catalog, &choices), ["timer"]);
    assert!(find(&catalog, &[], "timer nonexistent").is_empty());
}

#[test]
fn palette_ties_use_translated_titles() {
    let catalog = [node("a", "A", "Data", ""), node("z", "Z", "Data", "")];
    let result = ranked(&catalog, &[], "", |d| {
        (
            if d.type_name == "a" {
                "Z".into()
            } else {
                "A".into()
            },
            d.category.clone(),
            String::new(),
        )
    });
    assert_eq!(names(&catalog, &result), ["z", "a"]);
}

#[test]
fn recent_update_is_newest_first_deduplicated_and_limited_to_five() {
    let mut recent = Vec::new();
    for name in ["a", "b", "c", "d", "e", "f"] {
        recent = update_recent(&recent, name);
    }
    assert_eq!(recent, ["f", "e", "d", "c", "b"]);
    assert_eq!(update_recent(&recent, "c"), ["c", "f", "e", "d", "b"]);
    let stored = ["a".into(), "b".into(), "a".into(), String::new()];
    assert_eq!(update_recent(&stored, "b"), ["b", "a"]);
}

#[test]
fn at_most_five_recents_are_promoted() {
    let catalog: Vec<_> = (0..7)
        .map(|n| node(&n.to_string(), &n.to_string(), "Data", ""))
        .collect();
    let recent: Vec<_> = (0..7).rev().map(|n| n.to_string()).collect();
    let result = find(&catalog, &recent, "");
    assert_eq!(
        names(&catalog, &result),
        ["6", "5", "4", "3", "2", "0", "1"]
    );
    assert_eq!(result.iter().filter(|c| c.recent).count(), MAX_RECENT);
}
