use super::*;

#[test]
fn test_client_create_set_delete_variable() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, mut rx) = mpsc::unbounded_channel();

    let key = VariableKey::global("vol");
    let var = Variable {
        key: key.clone(),
        data_type: DataType::Number,
        value: DataValue::from(10),
    };

    // 1. Create variable
    actor
        .apply(ClientMessage::CreateVariable(var.clone()), None, &reply)
        .unwrap();
    assert_eq!(
        actor.engine.variables.get(&key).map(|v| &v.value),
        Some(&DataValue::from(10))
    );

    // Duplicate create fails with exists
    let err = actor
        .apply(ClientMessage::CreateVariable(var), None, &reply)
        .unwrap_err();
    assert_eq!(err.key, "error-variable-exists");

    // 2. Set variable valid
    actor
        .apply(
            ClientMessage::SetVariable {
                key: key.clone(),
                value: DataValue::from(20),
            },
            None,
            &reply,
        )
        .unwrap();
    assert_eq!(
        actor.engine.variables.get(&key).map(|v| &v.value),
        Some(&DataValue::from(20))
    );

    // 3. Set variable with inconvertible type fails
    let err = actor
        .apply(
            ClientMessage::SetVariable {
                key: key.clone(),
                value: DataValue::from("not_a_number"),
            },
            None,
            &reply,
        )
        .unwrap_err();
    assert_eq!(err.key, "error-variable-type");

    // 4. Set missing variable fails
    let err = actor
        .apply(
            ClientMessage::SetVariable {
                key: VariableKey::global("nonexistent"),
                value: DataValue::from(1),
            },
            None,
            &reply,
        )
        .unwrap_err();
    assert_eq!(err.key, "error-variable-missing");

    // 5. Delete variable
    actor
        .apply(ClientMessage::DeleteVariable(key.clone()), None, &reply)
        .unwrap();
    assert!(actor.engine.variables.get(&key).is_none());

    // Verify reply receiver has no errors sent directly (errors were returned via Result)
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_persistent_variables_save_on_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = mpsc::unbounded_channel();

    let key = VariableKey::persistent("saved_theme");
    let var = Variable {
        key: key.clone(),
        data_type: DataType::String,
        value: DataValue::from("nord"),
    };

    actor
        .apply(ClientMessage::CreateVariable(var), None, &reply)
        .unwrap();
    assert!(actor.engine.variables.is_persistent_dirty());

    actor.save();
    assert!(!actor.engine.variables.is_persistent_dirty());
    assert!(actor.storage.variables_json_path().exists());

    let loaded = actor.storage.load_variables().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].key.name, "saved_theme");
    assert_eq!(loaded[0].value, DataValue::from("nord"));
}

#[test]
fn test_sheet_deletion_drops_sheet_variables() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = mpsc::unbounded_channel();

    let sheet_id = actor
        .create_sheet("Scenario".into(), "".into(), &reply)
        .unwrap();
    let key = VariableKey::sheet(sheet_id, "local_var");
    let var = Variable {
        key: key.clone(),
        data_type: DataType::Bool,
        value: DataValue::Bool(true),
    };

    actor
        .apply(ClientMessage::CreateVariable(var), Some(sheet_id), &reply)
        .unwrap();
    assert!(actor.engine.variables.get(&key).is_some());

    actor.delete_sheet(sheet_id).unwrap();
    assert!(actor.engine.variables.get(&key).is_none());
}

#[test]
fn test_variables_changed_telemetry_filtering_by_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = mpsc::unbounded_channel();
    let sheet_id = actor.sheets[0].id;

    // Create a sheet variable and a global variable
    actor
        .apply(
            ClientMessage::CreateVariable(Variable {
                key: VariableKey::sheet(sheet_id, "s_var"),
                data_type: DataType::Number,
                value: DataValue::from(1),
            }),
            Some(sheet_id),
            &reply,
        )
        .unwrap();

    actor
        .apply(
            ClientMessage::CreateVariable(Variable {
                key: VariableKey::global("g_var"),
                data_type: DataType::Number,
                value: DataValue::from(2),
            }),
            None,
            &reply,
        )
        .unwrap();

    let flush = actor.telemetry.flush(
        Instant::now() + Duration::from_secs(1),
        &mut actor.engine,
        &actor.node_sheet,
    );
    let mut sheet_messages = Vec::new();
    let mut global_messages = Vec::new();

    for (tag, msg) in flush {
        if let ServerMessage::VariablesChanged { set, removed } = msg {
            if tag == Some(sheet_id) {
                sheet_messages.push((set, removed));
            } else if tag == None {
                global_messages.push((set, removed));
            }
        }
    }

    assert_eq!(sheet_messages.len(), 1);
    assert_eq!(sheet_messages[0].0.len(), 1);
    assert_eq!(sheet_messages[0].0[0].key.name, "s_var");

    assert_eq!(global_messages.len(), 1);
    assert_eq!(global_messages[0].0.len(), 1);
    assert_eq!(global_messages[0].0[0].key.name, "g_var");
}

#[test]
fn test_snapshot_init_filters_variables() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = mpsc::unbounded_channel();

    let s1 = actor.sheets[0].id;
    let s2 = actor
        .create_sheet("Second".into(), "".into(), &reply)
        .unwrap();

    actor.engine.variables.create(
        VariableKey::sheet(s1, "var1"),
        DataType::Number,
        DataValue::from(1),
    );
    actor.engine.variables.create(
        VariableKey::sheet(s2, "var2"),
        DataType::Number,
        DataValue::from(2),
    );
    actor.engine.variables.create(
        VariableKey::global("g_var"),
        DataType::Number,
        DataValue::from(3),
    );

    // Snapshot for s1
    let snap_s1 = actor.snapshot(Some(s1));
    let ServerMessage::Init {
        variables: vars_s1, ..
    } = snap_s1
    else {
        panic!()
    };
    let names_s1: Vec<_> = vars_s1.iter().map(|v| v.key.name.as_str()).collect();
    assert!(names_s1.contains(&"var1"));
    assert!(names_s1.contains(&"g_var"));
    assert!(!names_s1.contains(&"var2"));

    // Snapshot for home page (None)
    let snap_home = actor.snapshot(None);
    let ServerMessage::Init {
        variables: vars_home,
        ..
    } = snap_home
    else {
        panic!()
    };
    let names_home: Vec<_> = vars_home.iter().map(|v| v.key.name.as_str()).collect();
    assert!(names_home.contains(&"g_var"));
    assert!(!names_home.contains(&"var1"));
    assert!(!names_home.contains(&"var2"));
}
