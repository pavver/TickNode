use super::*;

#[test]
fn create_and_shared_global_persistent_namespace() {
    let mut store = VariableStore::new();
    assert!(store.create(VariableKey::global("msg"), DataType::String, "hello".into()));
    // Cannot create persistent with same name because namespace is shared
    assert!(!store.create(VariableKey::persistent("msg"), DataType::String, "world".into()));
    // Cannot create global with same name again
    assert!(!store.create(VariableKey::global("msg"), DataType::String, "again".into()));
    assert_eq!(store.len(), 1);
}

#[test]
fn sheet_shadowing_and_lookups() {
    let mut store = VariableStore::new();
    let s1 = SheetId::generate();
    let s2 = SheetId::generate();

    assert!(store.create(VariableKey::global("val"), DataType::Number, 10.into()));
    assert!(store.create(VariableKey::sheet(s1, "val"), DataType::Number, 20.into()));

    // Auto lookup on s1 finds sheet variable (shadowing global)
    let r1 = store.resolve("val", VariableLookup::Auto, Some(s1)).unwrap();
    assert_eq!(r1.scope, VariableScope::Sheet);
    assert_eq!(store.get(&r1).unwrap().value, DataValue::Number(20.into()));

    // Auto lookup on s2 finds global variable
    let r2 = store.resolve("val", VariableLookup::Auto, Some(s2)).unwrap();
    assert_eq!(r2.scope, VariableScope::Global);
    assert_eq!(store.get(&r2).unwrap().value, DataValue::Number(10.into()));

    // Explicit global lookup on s1 bypasses shadow
    let rg = store.resolve("val", VariableLookup::Global, Some(s1)).unwrap();
    assert_eq!(rg.scope, VariableScope::Global);

    // Explicit sheet lookup on s2 is None
    assert!(store.resolve("val", VariableLookup::Sheet, Some(s2)).is_none());
}

#[test]
fn change_tracking_coalesces_and_handles_deletion() {
    let mut store = VariableStore::new();
    let key = VariableKey::global("count");
    store.create(key.clone(), DataType::Number, 1.into());
    store.set(&key, 2.into());
    store.set(&key, 3.into());

    let changes = store.take_changes();
    assert_eq!(changes.set.len(), 1);
    assert_eq!(changes.set[0].value, DataValue::Number(3.into()));
    assert!(changes.removed.is_empty());

    // Subsequent take is empty
    assert!(store.take_changes().set.is_empty());

    // Delete tracks removed
    store.delete(&key);
    let changes2 = store.take_changes();
    assert!(changes2.set.is_empty());
    assert_eq!(changes2.removed, vec![key]);
}

#[test]
fn remove_sheet_clears_only_that_sheet() {
    let mut store = VariableStore::new();
    let s1 = SheetId::generate();
    let s2 = SheetId::generate();

    store.create(VariableKey::sheet(s1, "a"), DataType::Number, 1.into());
    store.create(VariableKey::sheet(s2, "b"), DataType::Number, 2.into());
    store.create(VariableKey::global("c"), DataType::Number, 3.into());

    store.remove_sheet(s1);
    assert!(store.get(&VariableKey::sheet(s1, "a")).is_none());
    assert!(store.get(&VariableKey::sheet(s2, "b")).is_some());
    assert!(store.get(&VariableKey::global("c")).is_some());
}

#[test]
fn persistent_dirty_flag_updates() {
    let mut store = VariableStore::new();
    let pk = VariableKey::persistent("persist");
    let gk = VariableKey::global("temp");

    assert!(!store.is_persistent_dirty());
    store.create(gk.clone(), DataType::Number, 1.into());
    assert!(!store.is_persistent_dirty());

    store.create(pk.clone(), DataType::Number, 1.into());
    assert!(store.is_persistent_dirty());
    store.clear_persistent_dirty();

    // Setting same value doesn't dirty
    store.set(&pk, 1.into());
    assert!(!store.is_persistent_dirty());

    // Setting different value dirties
    store.set(&pk, 2.into());
    assert!(store.is_persistent_dirty());
    store.clear_persistent_dirty();

    store.delete(&pk);
    assert!(store.is_persistent_dirty());
}
