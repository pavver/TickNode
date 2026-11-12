use super::*;
use ticknode_shared::{DataType, DataValue, Variable, VariableKey};

#[test]
fn fresh_storage_creates_main_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());
    let mut engine = TickEngine::new();

    let loaded = storage.load(&mut engine).unwrap();
    assert_eq!(loaded.sheets.len(), 1);
    assert_eq!(loaded.sheets[0].title, "Main");
    assert!(loaded.sheets[0].enabled);
    assert!(storage.sheets_json_path().exists());
    assert!(storage.sheet_path(loaded.sheets[0].id).exists());
}

#[test]
fn migration_from_old_graph_json_creates_main_sheet_and_renames_to_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let old_graph_path = dir.path().join("graph.json");

    let mut engine = TickEngine::new();
    let c = NodeId::generate();
    engine
        .create_node(
            c,
            "const",
            &[("value".into(), DataValue::String("hello".into()))],
        )
        .unwrap();
    let mut model = engine.export_graph();
    model.nodes[0].position = Position { x: 12.0, y: 34.0 };
    std::fs::write(
        &old_graph_path,
        serde_json::to_string_pretty(&model).unwrap(),
    )
    .unwrap();

    let storage = Storage::new(dir.path().to_path_buf());
    let mut restored = TickEngine::new();
    let loaded = storage.load(&mut restored).unwrap();

    assert_eq!(loaded.sheets.len(), 1);
    assert_eq!(loaded.sheets[0].title, "Main");
    assert!(loaded.sheets[0].enabled);
    assert_eq!(loaded.positions[&c], Position { x: 12.0, y: 34.0 });
    assert!(restored.has_node(c));

    // graph.json renamed to graph.json.migrated
    assert!(!old_graph_path.exists());
    assert!(dir.path().join("graph.json.migrated").exists());
}

#[test]
fn load_and_save_round_trip_of_two_sheets() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());

    let mut engine = TickEngine::new();
    let s1 = SheetId::generate();
    let s2 = SheetId::generate();
    let info1 = SheetInfo {
        id: s1,
        title: "Sheet 1".into(),
        caption: "Cap 1".into(),
        enabled: true,
    };
    let info2 = SheetInfo {
        id: s2,
        title: "Sheet 2".into(),
        caption: "Cap 2".into(),
        enabled: false,
    };

    let n1 = NodeId::generate();
    let n2 = NodeId::generate();
    engine
        .create_node(n1, "const", &[("value".into(), 1i64.into())])
        .unwrap();
    engine
        .create_node(n2, "const", &[("value".into(), 2i64.into())])
        .unwrap();

    let mut g1 = GraphModel::default();
    g1.nodes.push(engine.node_model(n1).unwrap());
    g1.nodes[0].position = Position { x: 10.0, y: 20.0 };

    let mut g2 = GraphModel::default();
    g2.nodes.push(engine.node_model(n2).unwrap());
    g2.nodes[0].position = Position { x: 100.0, y: 200.0 };

    storage
        .save_sheets_meta(&[info1.clone(), info2.clone()])
        .unwrap();
    storage.save_sheet(s1, &g1).unwrap();
    storage.save_sheet(s2, &g2).unwrap();

    let mut restored = TickEngine::new();
    let loaded = storage.load(&mut restored).unwrap();

    assert_eq!(loaded.sheets, vec![info1, info2]);
    assert_eq!(loaded.node_sheet[&n1], s1);
    assert_eq!(loaded.node_sheet[&n2], s2);
    assert_eq!(loaded.positions[&n1], Position { x: 10.0, y: 20.0 });
    assert_eq!(loaded.positions[&n2], Position { x: 100.0, y: 200.0 });
    assert!(!restored.is_disabled(n1));
    assert!(
        restored.is_disabled(n2),
        "sheet 2 was disabled, so node 2 must be disabled"
    );
}

#[test]
fn delete_sheet_moves_file_to_trash() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());

    let id = SheetId::generate();
    let g = GraphModel::default();
    storage.save_sheet(id, &g).unwrap();
    let file = storage.sheet_path(id);
    assert!(file.exists());

    storage.delete_sheet(id).unwrap();
    assert!(!file.exists());

    let trash_files: Vec<_> = std::fs::read_dir(storage.trash_dir()).unwrap().collect();
    assert_eq!(trash_files.len(), 1);
    let name = trash_files[0]
        .as_ref()
        .unwrap()
        .file_name()
        .to_string_lossy()
        .to_string();
    assert!(name.starts_with(&format!("{}-", id.0)));
    assert!(name.ends_with(".json"));
}

#[test]
fn corrupt_sheet_file_fails_load() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());
    let id = SheetId::generate();
    let info = SheetInfo {
        id,
        title: "Broken".into(),
        caption: "".into(),
        enabled: true,
    };
    storage.save_sheets_meta(&[info]).unwrap();
    std::fs::create_dir_all(storage.sheets_dir()).unwrap();
    std::fs::write(storage.sheet_path(id), "{ broken json").unwrap();

    let mut engine = TickEngine::new();
    assert!(storage.load(&mut engine).is_err());
}

#[test]
fn save_and_load_variables_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());

    let v1 = Variable {
        key: VariableKey::persistent("theme"),
        data_type: DataType::String,
        value: DataValue::from("dark"),
    };
    let v2 = Variable {
        key: VariableKey::persistent("brightness"),
        data_type: DataType::Number,
        value: DataValue::from(80),
    };

    storage.save_variables(&[v1.clone(), v2.clone()]).unwrap();
    assert!(storage.variables_json_path().exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(storage.variables_json_path())
            .unwrap()
            .permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    let loaded = storage.load_variables().unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].key.name, "brightness"); // sorted by name
    assert_eq!(loaded[1].key.name, "theme");

    let mut engine = TickEngine::new();
    storage.load(&mut engine).unwrap();
    assert_eq!(
        engine
            .variables
            .get(&VariableKey::persistent("theme"))
            .map(|v| &v.value),
        Some(&DataValue::from("dark"))
    );
    assert_eq!(
        engine
            .variables
            .get(&VariableKey::persistent("brightness"))
            .map(|v| &v.value),
        Some(&DataValue::from(80))
    );
}

#[test]
fn corrupt_variables_file_fails_load() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());
    std::fs::write(storage.variables_json_path(), "{ not valid json").unwrap();

    let mut engine = TickEngine::new();
    assert!(storage.load(&mut engine).is_err());
}

#[test]
fn settings_fallback_to_env_and_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");

    let no_env = Storage::load_settings_from(&path, &|_| None).unwrap();
    assert_eq!(
        no_env,
        ServerSettings {
            tick_ms: 100,
            tick_budget: 0,
            ..Default::default()
        }
    );

    let env = |k: &str| match k {
        "TICKNODE_TICK_MS" => Some("50".into()),
        "TICKNODE_TICK_BUDGET" => Some("1000".into()),
        _ => None,
    };
    let with_env = Storage::load_settings_from(&path, &env).unwrap();
    assert_eq!(
        with_env,
        ServerSettings {
            tick_ms: 50,
            tick_budget: 1000,
            ..Default::default()
        }
    );
}

#[test]
fn settings_file_precedence_over_env() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().to_path_buf());
    let path = storage.settings_json_path();

    storage
        .save_settings(&ServerSettings {
            tick_ms: 250,
            tick_budget: 2000,
            ..Default::default()
        })
        .unwrap();

    let env = |k: &str| match k {
        "TICKNODE_TICK_MS" => Some("50".into()),
        "TICKNODE_TICK_BUDGET" => Some("1000".into()),
        _ => None,
    };
    let loaded = Storage::load_settings_from(&path, &env).unwrap();
    assert_eq!(
        loaded,
        ServerSettings {
            tick_ms: 250,
            tick_budget: 2000,
            ..Default::default()
        }
    );
}

#[test]
fn settings_partial_file_uses_env_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{"tick_ms": 300}"#).unwrap();

    let env = |k: &str| match k {
        "TICKNODE_TICK_BUDGET" => Some("500".into()),
        _ => None,
    };
    let loaded = Storage::load_settings_from(&path, &env).unwrap();
    assert_eq!(
        loaded,
        ServerSettings {
            tick_ms: 300,
            tick_budget: 500,
            ..Default::default()
        }
    );
}

#[test]
fn settings_validation_clamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{"tick_ms": 0, "tick_budget": 5000000}"#).unwrap();

    let loaded = Storage::load_settings_from(&path, &|_| None).unwrap();
    assert_eq!(
        loaded,
        ServerSettings {
            tick_ms: 1,
            tick_budget: 1_000_000,
            ..Default::default()
        }
    );
}

#[test]
fn corrupt_settings_file_fails_start() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "corrupt json {").unwrap();

    let res = Storage::load_settings_from(&path, &|_| None);
    assert!(res.is_err());
}

#[test]
fn display_settings_roundtrip_and_invalid_file_is_rejected() {
    use ticknode_shared::{DisplayRules, QuantityDisplay, UnitStep};
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().join("graph.json"));
    let mut settings = ServerSettings {
        display: DisplayRules {
            quantities: [(
                "quantity.power".into(),
                QuantityDisplay {
                    digits: 4,
                    steps: vec![
                        UnitStep {
                            from: 0.0,
                            unit: "watt".into(),
                        },
                        UnitStep {
                            from: 10000.0,
                            unit: "kilowatt".into(),
                        },
                    ],
                    rounding: Default::default(),
                },
            )]
            .into(),
            ..Default::default()
        },
        ..Default::default()
    };
    storage.save_settings(&settings).unwrap();
    assert_eq!(
        Storage::load_settings_from(&storage.settings_json_path(), &|_| None).unwrap(),
        settings
    );
    settings
        .display
        .quantities
        .get_mut("quantity.power")
        .unwrap()
        .steps[0]
        .from = 1.0;
    storage.save_settings(&settings).unwrap();
    assert!(Storage::load_settings_from(&storage.settings_json_path(), &|_| None).is_err());
}
