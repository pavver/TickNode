use super::*;
use ticknode_plugin_sdk::*;

fn inputs_from(pairs: &[(u32, DataValue)]) -> Inputs {
    pairs
        .iter()
        .map(|(pin, val)| (PinId(*pin), val.clone()))
        .collect()
}

fn sample_media(kind: &str, file_id: Option<&str>, url: Option<&str>) -> DataValue {
    let mut builder = StructValue::new(&format!("media.{kind}"));
    if let Some(id) = file_id {
        builder = builder.field("file_id", id);
    }
    if let Some(u) = url {
        builder = builder.field("url", u);
    }
    builder.build()
}

#[test]
fn button_data_defaulting_and_url_required() {
    let mut btn_node = ButtonNode::new();

    // Default action is callback: missing data falls back to text
    let inputs = inputs_from(&[(1, DataValue::String("Submit".into()))]);
    let outputs = btn_node.execute(Trigger::Pin(PinId(1)), &inputs);
    assert_eq!(outputs.len(), 1);
    let Output::Data(pin, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(*pin, PinId(0));
    assert_eq!(field_string(val, "text"), Some("Submit"));
    assert_eq!(field_string(val, "action"), Some("callback"));
    assert_eq!(field_string(val, "data"), Some("Submit"));

    // Callback with explicit data
    let inputs_with_data = inputs_from(&[
        (1, DataValue::String("Submit".into())),
        (2, DataValue::String("custom_data".into())),
    ]);
    let outputs = btn_node.execute(Trigger::Pin(PinId(1)), &inputs_with_data);
    assert_eq!(outputs.len(), 1);
    let Output::Data(_, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val, "data"), Some("custom_data"));

    // Action url: missing URL -> no output
    btn_node.set_property("action", "url".into());
    let outputs_no_url = btn_node.execute(Trigger::Pin(PinId(1)), &inputs);
    assert!(outputs_no_url.is_empty());

    // Action url: with URL -> output button with data=url
    let inputs_url = inputs_from(&[
        (1, DataValue::String("Docs".into())),
        (2, DataValue::String("https://example.org".into())),
    ]);
    let outputs_url = btn_node.execute(Trigger::Pin(PinId(1)), &inputs_url);
    assert_eq!(outputs_url.len(), 1);
    let Output::Data(_, val) = &outputs_url[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val, "action"), Some("url"));
    assert_eq!(field_string(val, "data"), Some("https://example.org"));
}

#[test]
fn keyboard_row_filling_by_columns_and_string_inputs() {
    let mut kb_node = KeyboardNode::new();
    kb_node.set_property("kind", "inline".into());
    kb_node.set_property("count", "5".into());
    kb_node.set_property("columns", "2".into());

    let inputs = inputs_from(&[
        (1, DataValue::String("A".into())),
        (2, DataValue::String("B".into())),
        (3, DataValue::String("C".into())),
        (4, DataValue::String(String::new())), // empty input -> skipped
        (5, DataValue::String("D".into())),
    ]);

    let outputs = kb_node.execute(Trigger::Pin(PinId(0)), &inputs);
    assert_eq!(outputs.len(), 1);
    let Output::Data(pin, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(*pin, PinId(0));
    assert_eq!(field_string(val, "kind"), Some("inline"));

    let rows_val = field(val, "rows").expect("rows present");
    let DataValue::List { elements: rows, .. } = rows_val else {
        panic!("rows is a list");
    };
    // 4 non-empty buttons with columns=2 -> 2 rows
    assert_eq!(rows.len(), 2);

    let DataValue::List {
        elements: row0_btns,
        ..
    } = &rows[0]
    else {
        panic!("row0 is a list");
    };
    assert_eq!(row0_btns.len(), 2);
    assert_eq!(field_string(&row0_btns[0], "text"), Some("A"));
    assert_eq!(field_string(&row0_btns[0], "action"), Some("callback"));
    assert_eq!(field_string(&row0_btns[0], "data"), Some("A"));
    assert_eq!(field_string(&row0_btns[1], "text"), Some("B"));

    let DataValue::List {
        elements: row1_btns,
        ..
    } = &rows[1]
    else {
        panic!("row1 is a list");
    };
    assert_eq!(row1_btns.len(), 2);
    assert_eq!(field_string(&row1_btns[0], "text"), Some("C"));
    assert_eq!(field_string(&row1_btns[1], "text"), Some("D"));
}

#[test]
fn keyboard_inline_reply_validation_remove_and_reply_options() {
    let mut kb_node = KeyboardNode::new();

    // Inline rejects contact button object
    kb_node.set_property("kind", "inline".into());
    let contact_btn = StructValue::new("telegram.button")
        .field("text", "Share phone")
        .field("action", "contact")
        .build();
    let inputs_contact = inputs_from(&[(1, contact_btn)]);
    let outputs = kb_node.execute(Trigger::Pin(PinId(0)), &inputs_contact);
    assert!(outputs.is_empty(), "inline must reject contact button");

    // Reply rejects callback button object
    kb_node.set_property("kind", "reply".into());
    let callback_btn = StructValue::new("telegram.button")
        .field("text", "Click me")
        .field("action", "callback")
        .field("data", "click")
        .build();
    let inputs_cb = inputs_from(&[(1, callback_btn)]);
    let outputs = kb_node.execute(Trigger::Pin(PinId(0)), &inputs_cb);
    assert!(outputs.is_empty(), "reply must reject callback button");

    // Reply keyboard with string input creates text button and sets reply options
    kb_node.set_property("resize", "yes".into());
    kb_node.set_property("one_time", "yes".into());
    kb_node.set_property("placeholder", "Prompt here".into());
    kb_node.set_property("persistent", "no".into());

    let inputs_text = inputs_from(&[(1, DataValue::String("Option 1".into()))]);
    let outputs = kb_node.execute(Trigger::Pin(PinId(0)), &inputs_text);
    assert_eq!(outputs.len(), 1);
    let Output::Data(_, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val, "kind"), Some("reply"));
    assert_eq!(field_bool(val, "resize"), Some(true));
    assert_eq!(field_bool(val, "one_time"), Some(true));
    assert_eq!(field_string(val, "placeholder"), Some("Prompt here"));
    assert_eq!(field_bool(val, "persistent"), Some(false));

    let rows_val = field(val, "rows").expect("rows present");
    let DataValue::List { elements: rows, .. } = rows_val else {
        panic!("rows is a list");
    };
    let DataValue::List { elements: btns, .. } = &rows[0] else {
        panic!("row0 is a list");
    };
    assert_eq!(field_string(&btns[0], "text"), Some("Option 1"));
    assert_eq!(field_string(&btns[0], "action"), Some("text"));

    // Remove keyboard outputs remove kind and empty rows, ignoring buttons
    kb_node.set_property("kind", "remove".into());
    let outputs_remove = kb_node.execute(Trigger::Pin(PinId(0)), &inputs_text);
    assert_eq!(outputs_remove.len(), 1);
    let Output::Data(_, val_remove) = &outputs_remove[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val_remove, "kind"), Some("remove"));
    let rows_val = field(val_remove, "rows").expect("rows present");
    let DataValue::List { elements: rows, .. } = rows_val else {
        panic!("rows is a list");
    };
    assert!(rows.is_empty());
}

#[test]
fn compose_absent_fields_text_caption_limits_and_media_validation() {
    let mut compose = ComposeNode::new();

    // Error without text and media
    let empty_inputs = inputs_from(&[]);
    let outputs_empty = compose.execute(Trigger::Pin(PinId(0)), &empty_inputs);
    assert!(outputs_empty.is_empty());

    // Absent fields stay absent (plain text only: parse_mode, media, keyboard, silent absent)
    let text_inputs = inputs_from(&[(1, DataValue::String("Just text".into()))]);
    let outputs = compose.execute(Trigger::Pin(PinId(0)), &text_inputs);
    assert_eq!(outputs.len(), 1);
    let Output::Data(_, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val, "text"), Some("Just text"));
    assert!(field(val, "parse_mode").is_none());
    assert!(field(val, "media").is_none());
    assert!(field(val, "keyboard").is_none());
    assert!(field(val, "silent").is_none());

    // Non-default parse_mode and silent are included
    compose.set_property("parse_mode", "html".into());
    compose.set_property("silent", "yes".into());
    let outputs = compose.execute(Trigger::Pin(PinId(0)), &text_inputs);
    let Output::Data(_, val) = &outputs[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val, "parse_mode"), Some("html"));
    assert_eq!(field_bool(val, "silent"), Some(true));

    // Text limit without media: > 4096 fails
    let long_inputs = inputs_from(&[(1, DataValue::String("a".repeat(4097)))]);
    let outputs_long = compose.execute(Trigger::Pin(PinId(0)), &long_inputs);
    assert!(outputs_long.is_empty());

    // Caption limit with media: > 1024 fails
    let photo = sample_media("photo", Some("photo_file"), None);
    let caption_inputs =
        inputs_from(&[(1, DataValue::String("a".repeat(1025))), (2, photo.clone())]);
    let outputs_cap = compose.execute(Trigger::Pin(PinId(0)), &caption_inputs);
    assert!(outputs_cap.is_empty());

    // Valid caption with media passes
    let valid_caption_inputs = inputs_from(&[
        (1, DataValue::String("Nice photo".into())),
        (2, photo.clone()),
    ]);
    let outputs_valid = compose.execute(Trigger::Pin(PinId(0)), &valid_caption_inputs);
    assert_eq!(outputs_valid.len(), 1);
    let Output::Data(_, val_valid) = &outputs_valid[0] else {
        panic!("expected data output");
    };
    assert_eq!(field_string(val_valid, "text"), Some("Nice photo"));
    assert!(field(val_valid, "media").is_some());

    // Media validation: invalid object missing url and file_id fails
    let bad_media = StructValue::new("media.video").build();
    let bad_media_inputs = inputs_from(&[(2, bad_media)]);
    let outputs_bad = compose.execute(Trigger::Pin(PinId(0)), &bad_media_inputs);
    assert!(outputs_bad.is_empty());
}

#[test]
fn compose_accepts_blob_media_for_send() {
    let mut node = ComposeNode::new();
    let media = StructValue::new("media.photo")
        .field("blob", "camera-blob")
        .build();
    let values = inputs_from(&[(1, "Camera".into()), (2, media.clone())]);
    let outputs = node.execute(Trigger::Pin(PinId(0)), &values);
    let Output::Data(_, outgoing) = &outputs[0] else {
        panic!("Expected composed message");
    };
    assert_eq!(field(outgoing, "media"), Some(&media));
    assert!(
        matches!(crate::outgoing::parse_outgoing(outgoing).unwrap().media.unwrap().source, crate::outgoing::MediaSource::Blob(id) if id == "camera-blob")
    );
}
