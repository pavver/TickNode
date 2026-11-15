use super::*;
use std::collections::BTreeMap;
use ticknode_shared::media::*;

#[test]
fn media_types_registered_at_engine_construction() {
    let engine = TickEngine::new();
    for name in [PHOTO, AUDIO, VIDEO, DOCUMENT] {
        let def = engine
            .types
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(def.is_struct());
        assert_eq!(def.fields().len(), 9);
        assert_eq!(def.fields()[0].name, "url");
        assert_eq!(def.fields()[1].name, "file_id");
        assert_eq!(def.fields()[2].name, "mime_type");
        assert_eq!(def.fields()[3].name, "file_name");
        assert_eq!(def.fields()[4].name, "size");
        assert_eq!(def.fields()[5].name, "width");
        assert_eq!(def.fields()[6].name, "height");
        assert_eq!(def.fields()[7].name, "duration");
        assert_eq!(def.fields()[8].name, "blob");
    }
}

#[test]
fn reserved_media_prefix_rejected_for_plugins() {
    let mut engine = TickEngine::new();
    let custom = TypeDefinition::new_enum("media.custom", "Custom", &[("a", "A")]);
    let err = engine.register_types(&[custom], &[]).unwrap_err();
    assert!(err.contains("reserved media namespace"));
}

#[test]
fn make_and_break_media_nodes_work() {
    let mut engine = TickEngine::new();
    let make_id = NodeId::generate();
    let break_id = NodeId::generate();

    // Universal object/make and object/break for media.photo
    engine
        .create_node(
            make_id,
            "object/make",
            &[("type".into(), "media.photo".into())],
        )
        .unwrap();
    engine
        .create_node(
            break_id,
            "object/break",
            &[("type".into(), "media.photo".into())],
        )
        .unwrap();

    // Connect make output (pin 0) to break input (pin 0)
    engine
        .connect_data(make_id, PinId(0), break_id, PinId(0))
        .unwrap();

    // Set fields on Make node: url (pin 1), file_id (pin 2), width (pin 6), height (pin 7)
    engine
        .set_input_value(
            make_id,
            PinId(1),
            Some(DataValue::String("https://example.com/pic.jpg".into())),
        )
        .unwrap();
    engine
        .set_input_value(
            make_id,
            PinId(2),
            Some(DataValue::String("tg_file_42".into())),
        )
        .unwrap();
    engine
        .set_input_value(
            make_id,
            PinId(6),
            Some(DataValue::Number(NumberValue::Int(1920))),
        )
        .unwrap();
    engine
        .set_input_value(
            make_id,
            PinId(7),
            Some(DataValue::Number(NumberValue::Int(1080))),
        )
        .unwrap();

    let mut stack = Vec::new();
    // Pull from break node: url (pin 1), file_id (pin 2), width (pin 6), height (pin 7)
    assert_eq!(
        engine.pull_data(break_id, PinId(1), &mut stack),
        Some(DataValue::String("https://example.com/pic.jpg".into()))
    );
    assert_eq!(
        engine.pull_data(break_id, PinId(2), &mut stack),
        Some(DataValue::String("tg_file_42".into()))
    );
    assert_eq!(
        engine.pull_data(break_id, PinId(6), &mut stack),
        Some(DataValue::Number(NumberValue::Int(1920)))
    );
    assert_eq!(
        engine.pull_data(break_id, PinId(7), &mut stack),
        Some(DataValue::Number(NumberValue::Int(1080)))
    );
    // Optional field not set on Make has no value on Break output
    assert_eq!(engine.pull_data(break_id, PinId(3), &mut stack), None);
}

#[test]
fn universal_object_make_and_break_support_media_types() {
    let mut engine = TickEngine::new();
    let make_id = NodeId::generate();
    let break_id = NodeId::generate();

    engine
        .create_node(
            make_id,
            "object/make",
            &[("type".into(), "media.video".into())],
        )
        .unwrap();
    engine
        .create_node(
            break_id,
            "object/break",
            &[("type".into(), "media.video".into())],
        )
        .unwrap();

    engine
        .connect_data(make_id, PinId(0), break_id, PinId(0))
        .unwrap();

    engine
        .set_input_value(make_id, PinId(2), Some(DataValue::String("vid_123".into())))
        .unwrap();
    let delta = chrono::TimeDelta::seconds(45);
    engine
        .set_input_value(make_id, PinId(8), Some(DataValue::Duration(delta)))
        .unwrap();

    let mut stack = Vec::new();
    assert_eq!(
        engine.pull_data(break_id, PinId(2), &mut stack),
        Some(DataValue::String("vid_123".into()))
    );
    assert_eq!(
        engine.pull_data(break_id, PinId(8), &mut stack),
        Some(DataValue::Duration(delta))
    );
}

#[test]
fn media_validation_and_source_extraction() {
    let mut fields = BTreeMap::new();
    fields.insert(
        "file_id".into(),
        ObjectField {
            name: "file_id".into(),
            description: String::new(),
            value: DataValue::String("file_xyz".into()),
        },
    );
    let val = DataValue::Object {
        type_name: Some("media.document".into()),
        fields,
    };
    assert_eq!(
        validate_media(&val).unwrap(),
        MediaSource::FileId("file_xyz")
    );

    let empty = DataValue::Object {
        type_name: Some("media.document".into()),
        fields: BTreeMap::new(),
    };
    assert!(validate_media(&empty).is_err());
}
