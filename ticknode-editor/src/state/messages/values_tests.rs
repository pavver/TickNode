use super::*;
use ticknode_shared::{ValueCatalog, ValueEntrySummary, ValueField};

#[test]
fn field_answers_are_cached_and_only_changed_namespaces_are_invalidated() {
    let owner = Owner::new();
    owner.with(|| {
        let editor = Editor::new(Theme::Dark, RwSignal::new(Locale::En), true);
        let fields = vec![ValueField {
            path: "state".into(),
            title: "State".into(),
            data_type: DataType::Bool,
        }];
        for namespace in ["ha", "other"] {
            let answer = ServerMessage::ValueEntryFields {
                namespace: namespace.into(),
                key: "light.a".into(),
                fields: fields.clone(),
            };
            editor.apply(answer.clone());
            editor.apply(answer);
        }
        assert_eq!(editor.value_entry_fields.get().len(), 2);
        let change = ServerMessage::ValueCatalogs(vec![ValueCatalog {
            namespace: "ha".into(),
            entries: vec![ValueEntrySummary {
                key: "light.a".into(),
                title: "Lamp".into(),
                icon: None,
            }],
        }]);
        editor.apply(change.clone());
        editor.apply(change);
        let cache = editor.value_entry_fields.get();
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[&("other".into(), "light.a".into())], fields);
        assert_eq!(editor.value_catalogs.get().len(), 1);
        editor.apply(ServerMessage::ValueCatalogs(vec![ValueCatalog {
            namespace: "ha".into(),
            entries: vec![],
        }]));
        assert!(editor.value_catalogs.get().is_empty());
    });
}
