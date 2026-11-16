//! Small read-only namespace fixture with observable watch calls.
use std::cell::RefCell;
use ticknode_plugin_sdk::*;
thread_local! {
static LOGS: RefCell<Vec<String>> = RefCell::default(); static WATCHED: RefCell<std::collections::BTreeSet<String>> = RefCell::default(); }
pub fn configure() {
    host::set_value_catalog(&["sensor.one", "sensor.two"].map(|key| ValueEntry {
        key: key.into(),
        title: key.into(),
        icon: None,
        fields: vec![ValueField {
            path: "state".into(),
            title: "State".into(),
            data_type: DataType::Named("quantity.power".into()),
        }],
    }));
}
pub fn watch(added: Vec<String>, removed: Vec<String>) {
    for key in &added {
        LOGS.with(|logs| logs.borrow_mut().push(format!("Value watch added {key}")));
    }
    for key in &removed {
        LOGS.with(|logs| logs.borrow_mut().push(format!("Value watch removed {key}")));
    }
    WATCHED.with(|watched| {
        let mut watched = watched.borrow_mut();
        for key in removed {
            watched.remove(&key);
        }
        watched.extend(added);
    });
}
pub fn tick() {
    LOGS.with(|logs| {
        for log in logs.take() {
            host::log(&log);
        }
    });
    WATCHED.with(|watched| {
        for key in watched.borrow().iter() {
            let value = DataValue::Object {
                type_name: None,
                fields: [(
                    "state".into(),
                    ObjectField {
                        data_type: Some(DataType::Named("quantity.power".into())),
                        value: (host::current_tick() as i64 * 1000).into(),
                    },
                )]
                .into(),
            };
            host::set_value(key, Some(&value));
        }
    });
}
