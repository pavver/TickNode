use super::*;

fn registry(title: &str, field: &str) -> TypeRegistry {
    TypeRegistry::from_definitions([TypeDefinition::new_struct(
        "test.point",
        title,
        vec![FieldDefinition::new(field, field, DataType::Number)],
    )])
}

fn snapshot(title: &str, field: &str) -> DataValue {
    encode_fields_snapshot(
        title,
        &[FieldDefinition::new(field, field, DataType::Number)],
    )
    .unwrap()
    .into()
}

#[test]
fn live_definitions_override_restored_fields_and_changed_titles_refresh_revisions() {
    let mut object = ObjectType::default();
    object.types_changed(&registry("Point", "x"));
    object.set_property("type", "test.point".into());
    let revision = object.revision;
    object.set_property("fields", snapshot("Stale", "old"));
    assert_eq!(object.type_title, "Point");
    assert_eq!(object.fields_snapshot[0].name, "x");
    assert_eq!(object.revision, revision);
    object.types_changed(&registry("Renamed", "x"));
    assert_eq!(object.type_title, "Renamed");
    assert!(object.revision > revision);
    let revision = object.revision;
    object.types_changed(&registry("Renamed", "x"));
    assert_eq!(object.revision, revision);
    object.types_changed(&registry("Renamed", "y"));
    assert_eq!(object.fields_snapshot[0].name, "y");
    assert!(object.revision > revision);
}

#[test]
fn unknown_type_snapshots_accept_independent_title_and_field_changes() {
    let mut object = ObjectType::default();
    object.set_property("type", "missing.point".into());
    for (title, field) in [("Point", "x"), ("Renamed", "x"), ("Renamed", "y")] {
        let previous = object.revision;
        object.set_property("fields", snapshot(title, field));
        assert_eq!(object.type_title, title);
        assert_eq!(object.fields_snapshot[0].name, field);
        assert!(object.revision > previous);
    }
    let previous = object.revision;
    object.set_property("fields", snapshot("Renamed", "y"));
    assert_eq!(object.revision, previous);
    let PropertyKind::Select(options) = object.type_property("Choose").kind else {
        panic!("select")
    };
    assert_eq!(
        options,
        [
            ("".into(), "—".into()),
            ("missing.point".into(), "Renamed".into())
        ]
    );
    object.set_property("type", "other.missing".into());
    assert!(object.fields_snapshot.is_empty());
    object.set_property("type", "".into());
    let PropertyKind::Select(options) = object.type_property("Choose").kind else {
        panic!("select")
    };
    assert_eq!(options, [("".into(), "—".into())]);
}

#[test]
fn options_revision_changes_when_other_available_structs_change() {
    let mut object = ObjectType::default();
    let types = registry("Point", "x");
    object.types_changed(&types);
    let empty_revision = object.revision;
    assert!(empty_revision > 0);
    object.set_property("type", "test.point".into());
    let selected_revision = object.revision;
    object.types_changed(&TypeRegistry::new());
    assert!(object.revision > selected_revision);
    assert_eq!(object.type_title, "Point");
    object.types_changed(&types);
    let revision = object.revision;
    let mut extended = types;
    extended
        .insert_all(&[TypeDefinition::new_struct("test.extra", "Extra", vec![])])
        .unwrap();
    object.types_changed(&extended);
    assert!(object.revision > revision);
    let PropertyKind::Select(options) = object.type_property("Choose").kind else {
        panic!("select")
    };
    assert_eq!(options.len(), 3);
    assert_eq!(options[2], ("test.point".into(), "Point".into()));
}
