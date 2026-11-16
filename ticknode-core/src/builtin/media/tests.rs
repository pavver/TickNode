use super::*;
use crate::{HostState, TickEngine};
use std::collections::HashMap;

#[test]
fn fetch_reports_translatable_failures_and_clears_stale_media() {
    let engine = TickEngine::new();
    let mut host = HostState::default();
    let mut node = MediaFetchNode::new(NodeId::generate());
    let mut ctx = NodeContext {
        node_id: node.id,
        inputs: HashMap::new(),
        input_types: HashMap::new(),
        plugin_values: &crate::PluginValues::default(),
        variables: &engine.variables,
        types: &engine.types,
        sheet: None,
        host_state: &mut host,
    };
    assert!(node.execute(None, &mut ctx).is_empty());
    for value in [None, Some(DataValue::from("   ")), Some(4i64.into())] {
        ctx.inputs.insert(PinId(1), value);
        let outputs = node.execute(Some(PinId(0)), &mut ctx);
        assert!(outputs.contains(&NodeOutput::Execution(PinId(11))));
        assert!(outputs.contains(&NodeOutput::Clear(PinId(13))));
        assert_eq!(
            ctx.host_state.errors.last().unwrap().1.key,
            "error-media-fetch-url"
        );
    }
    ctx.inputs
        .insert(PinId(1), Some("https://example.com/image".into()));
    ctx.inputs.insert(
        PinId(2),
        Some("X-Test: value\n\ninvalid\nX-Other: a:b".into()),
    );
    node.execute(Some(PinId(0)), &mut ctx);
    assert_eq!(
        ctx.host_state.errors.last().unwrap().1.key,
        "error-media-fetch-transport"
    );
    for result in [
        Err("request failed".into()),
        Ok(TaskOutput::Http(crate::host::HttpResponse {
            status: 404,
            headers: vec![],
            body: vec![],
        })),
    ] {
        let outputs = node.on_task(1, result, &mut ctx);
        assert!(outputs.contains(&NodeOutput::Execution(PinId(11))));
        assert!(outputs.contains(&NodeOutput::Clear(PinId(13))));
        assert!(!ctx.host_state.errors.last().unwrap().1.key.is_empty());
    }
    assert!(node.on_task(1, Ok(TaskOutput::Delay), &mut ctx).is_empty());
    assert_eq!(
        parse_headers("X-Test: value\n\ninvalid\nX-Other: a:b"),
        [
            ("X-Test".into(), "value".into()),
            ("X-Other".into(), "a:b".into())
        ]
    );
}

#[test]
fn fetch_blob_completion_preserves_metadata_and_clears_previous_error_output() {
    let engine = TickEngine::new();
    let mut host = HostState::default();
    let info = host
        .blob_store
        .put(
            vec![1, 2, 3],
            Some("image/png".into()),
            Some("camera.png".into()),
        )
        .unwrap();
    let mut node = MediaFetchNode::new(NodeId::generate());
    node.set_property("kind", "document".into());
    let mut ctx = NodeContext {
        node_id: node.id,
        inputs: HashMap::new(),
        input_types: HashMap::new(),
        plugin_values: &crate::PluginValues::default(),
        variables: &engine.variables,
        types: &engine.types,
        sheet: None,
        host_state: &mut host,
    };
    let outputs = node.on_task(
        1,
        Ok(TaskOutput::Blob(crate::host::BlobResponse {
            status: 200,
            headers: vec![],
            blob: info.id.clone(),
            size: 3,
            content_type: Some("image/png".into()),
        })),
        &mut ctx,
    );
    assert!(outputs.contains(&NodeOutput::Clear(PinId(12))));
    assert!(outputs.contains(&NodeOutput::Execution(PinId(10))));
    let value = outputs
        .iter()
        .find_map(|output| match output {
            NodeOutput::Data(PinId(13), value) => Some(value),
            _ => None,
        })
        .unwrap();
    let DataValue::Object { type_name, fields } = value else {
        panic!("media object")
    };
    assert_eq!(type_name.as_deref(), Some("media.document"));
    assert_eq!(fields["blob"].value, DataValue::from(info.id));
    assert_eq!(fields["size"].value, 3i64.into());
    assert_eq!(fields["file_name"].value, "camera.png".into());
    assert_eq!(fields["mime_type"].value, "image/png".into());
}
