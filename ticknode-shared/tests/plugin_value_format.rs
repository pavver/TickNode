use chrono::{NaiveTime, TimeZone, Utc};
use serde_reflection::{Samples, Tracer, TracerConfig};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};
use ticknode_shared::{Color, DataType, DataValue, NumberValue, ObjectField};

const UPDATE_HINT: &str = "The plugin value format changed; plugins must be rebuilt. Run \
    UPDATE_PLUGIN_VALUE_FORMAT=1 cargo test -p ticknode-shared plugin_value_format to accept.";

#[test]
fn plugin_value_format() {
    // Postcard is not human-readable. Chrono validates strings during deserialization, so
    // sample all variants before tracing the type, avoiding synthetic invalid Chrono strings.
    let mut tracer = Tracer::new(TracerConfig::default().is_human_readable(false));
    let mut samples = Samples::new();
    tracer
        .trace_simple_type::<NumberValue>()
        .expect(UPDATE_HINT);
    tracer.trace_simple_type::<DataType>().expect(UPDATE_HINT);
    let fields = BTreeMap::from([(
        "field".into(),
        ObjectField {
            name: "field".into(),
            description: "Example field".into(),
            value: DataValue::Bool(true),
        },
    )]);
    for value in [
        DataValue::Bool(true),
        DataValue::Number(NumberValue::Int(42)),
        DataValue::Number(NumberValue::Float(1.5)),
        DataValue::String("value".into()),
        DataValue::Time(NaiveTime::from_hms_opt(12, 34, 56).unwrap()),
        DataValue::DateTime(Utc.with_ymd_and_hms(2026, 1, 2, 12, 34, 56).unwrap()),
        DataValue::Duration(Duration::new(1, 2)),
        DataValue::Color(Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        }),
        DataValue::List {
            element_type: DataType::Bool,
            elements: vec![DataValue::Bool(true)],
        },
        DataValue::Object {
            fields: fields.clone(),
            type_name: Some("test.value".into()),
        },
        DataValue::Object {
            fields,
            type_name: None,
        },
    ] {
        tracer.trace_value(&mut samples, &value).expect(UPDATE_HINT);
    }
    // This also detects newly added variants that have no serialization sample yet.
    tracer.trace_simple_type::<DataValue>().expect(UPDATE_HINT);

    // Registry and its variant maps are BTreeMaps, keeping the snapshot order stable.
    let registry = tracer.registry().expect(UPDATE_HINT);
    let actual = format!("{}\n", serde_json::to_string_pretty(&registry).unwrap());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin-value-format.json");
    if std::env::var("UPDATE_PLUGIN_VALUE_FORMAT").as_deref() == Ok("1") {
        fs::write(&path, &actual).expect("write plugin value format snapshot");
    }
    let expected = fs::read_to_string(path).expect("read plugin value format snapshot");
    assert_eq!(actual, expected, "{UPDATE_HINT}");
}
