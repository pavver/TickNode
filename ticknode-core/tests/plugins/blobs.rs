use std::sync::mpsc;

fn serve_raw_full(response: Vec<u8>) -> (u16, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = vec![0u8; 65536];
        let n = stream.read(&mut buf).unwrap_or(0);
        let _ = tx.send(buf[..n].to_vec());
        let _ = stream.write_all(&response);
    });
    (port, rx)
}

fn serve_big(body_size: usize, content_type: &str) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let ct = content_type.to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req_header = String::from_utf8_lossy(&buf[..n]).to_string();
        let _ = tx.send(req_header.lines().next().unwrap_or_default().to_string());

        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {body_size}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(header.as_bytes());
        let chunk = vec![0x42u8; 65536];
        let mut sent = 0;
        while sent < body_size {
            let to_send = (body_size - sent).min(chunk.len());
            if stream.write_all(&chunk[..to_send]).is_err() {
                break;
            }
            sent += to_send;
        }
    });
    (port, rx)
}

#[test]
fn host_http_multipart_with_text_and_blob_parts() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    let blob_info = engine
        .host_state
        .blob_store
        .put(
            b"secret binary media content".to_vec(),
            Some("application/octet-stream".into()),
            Some("data.bin".into()),
        )
        .unwrap();

    let (port, req_rx) = serve_raw_full(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK".to_vec());

    let req = HttpRequest {
        method: "POST".into(),
        url: format!("http://127.0.0.1:{port}/upload"),
        headers: vec![("X-Client".into(), "TickNode".into())],
        body: vec![],
        timeout: std::time::Duration::from_secs(5),
        multipart: vec![
            MultipartPart {
                name: "username".into(),
                file_name: None,
                content_type: None,
                data: PartData::Text("camera_admin".into()),
            },
            MultipartPart {
                name: "file".into(),
                file_name: Some("snapshot.bin".into()),
                content_type: Some("application/octet-stream".into()),
                data: PartData::Blob(blob_info.id),
            },
        ],
        save_to_blob: false,
    };

    let node_id = NodeId::generate();
    let task = engine.host_state.start_http(node_id, req).unwrap();

    run_until(&mut engine, |e| !e.host_state.active_tasks.contains_key(&task));

    let req_bytes = req_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    let req_str = String::from_utf8_lossy(&req_bytes);

    assert!(req_str.contains("POST /upload HTTP/1.1"));
    assert!(req_str.contains("x-client: TickNode"));
    assert!(req_str.contains("multipart/form-data; boundary="));
    assert!(req_str.contains("name=\"username\""));
    assert!(req_str.contains("camera_admin"));
    assert!(req_str.contains("name=\"file\""));
    assert!(req_str.contains("filename=\"snapshot.bin\""));
    assert!(req_str.contains("application/octet-stream"));
    assert!(req_str.contains("secret binary media content"));
}

#[test]
fn host_http_unknown_blob_in_multipart_returns_error() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    let req = HttpRequest {
        method: "POST".into(),
        url: "http://127.0.0.1:9/upload".into(),
        headers: vec![],
        body: vec![],
        timeout: std::time::Duration::from_secs(5),
        multipart: vec![MultipartPart {
            name: "file".into(),
            file_name: None,
            content_type: None,
            data: PartData::Blob("non-existent-blob-id".into()),
        }],
        save_to_blob: false,
    };

    let node_id = NodeId::generate();
    let err = engine.host_state.start_http(node_id, req).unwrap_err();
    assert_eq!(err, "unknown blob");
}

#[test]
fn host_http_save_to_blob_stores_body_and_handles_over_4mib() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    // 5 MiB payload — exceeds default 4 MiB limit for normal HTTP responses
    let size = 5 * 1024 * 1024;
    let (port, _rx) = serve_big(size, "video/mp4");

    let req = HttpRequest {
        method: "GET".into(),
        url: format!("http://127.0.0.1:{port}/stream.mp4"),
        headers: vec![],
        body: vec![],
        timeout: std::time::Duration::from_secs(10),
        multipart: vec![],
        save_to_blob: true,
    };

    let node_id = NodeId::generate();
    let task = engine.host_state.start_http(node_id, req).unwrap();

    let mut result = None;
    for _ in 0..300 {
        for (id, t, res) in engine.host_state.take_finished() {
            if id == node_id && t == task {
                result = Some(res);
                break;
            }
        }
        if result.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let task_output = result.expect("task did not complete").expect("task failed");
    let TaskOutput::Blob(blob_resp) = task_output else {
        panic!("expected Blob output");
    };

    assert_eq!(blob_resp.status, 200);
    assert_eq!(blob_resp.size, size as u64);
    assert_eq!(blob_resp.content_type.as_deref(), Some("video/mp4"));

    let stored_bytes = engine
        .host_state
        .blob_store
        .read_all(&blob_resp.blob)
        .unwrap();
    assert_eq!(stored_bytes.len(), size);
    assert_eq!(stored_bytes[0], 0x42);
}

#[test]
fn host_http_save_to_blob_non_2xx_returns_normal_http() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    let (port, _) = serve_raw("HTTP/1.1 404 Not Found\r\nContent-Length: 14\r\nConnection: close\r\n\r\nfile not found".into());

    let req = HttpRequest {
        method: "GET".into(),
        url: format!("http://127.0.0.1:{port}/missing.jpg"),
        headers: vec![],
        body: vec![],
        timeout: std::time::Duration::from_secs(5),
        multipart: vec![],
        save_to_blob: true,
    };

    let node_id = NodeId::generate();
    let task = engine.host_state.start_http(node_id, req).unwrap();

    let mut result = None;
    for _ in 0..100 {
        for (id, t, res) in engine.host_state.take_finished() {
            if id == node_id && t == task {
                result = Some(res);
                break;
            }
        }
        if result.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let task_output = result.expect("task did not complete").expect("task failed");
    let TaskOutput::Http(http_resp) = task_output else {
        panic!("expected Http output on non-2xx");
    };

    assert_eq!(http_resp.status, 404);
    assert_eq!(http_resp.body, b"file not found");
}

#[test]
fn media_fetch_node_success_and_error() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    let img_data = b"image-payload-bytes";
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Disposition: attachment; filename=\"camera.jpg\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        img_data.len(),
        std::str::from_utf8(img_data).unwrap()
    );
    let (port, _) = serve_raw(resp);

    let fetch_id = NodeId::generate();
    engine.create_node(fetch_id, "media/fetch", &[("kind".into(), "photo".into())]).unwrap();

    engine
        .set_input_value(
            fetch_id,
            PinId(1),
            Some(DataValue::String(format!("http://127.0.0.1:{port}/snapshot.jpg"))),
        )
        .unwrap();

    let done_sink = sink(&mut engine, fetch_id, 10);
    let _failed_sink = sink(&mut engine, fetch_id, 11);

    engine.trigger_node(fetch_id, PinId(0));

    run_until(&mut engine, |e| e.executed.iter().any(|t| t.node_id == done_sink));

    let media = output(&engine, fetch_id, 13).expect("Media output pin has value");
    assert!(matches!(media, DataValue::Object { ref type_name, .. } if type_name.as_deref() == Some("media.photo")));

    let source = ticknode_shared::media::validate_media(&media).unwrap();
    let blob_id = match source {
        ticknode_shared::media::MediaSource::Blob(id) => id,
        other => panic!("expected Blob source, got {other:?}"),
    };

    let stored = engine.host_state.blob_store.read_all(blob_id).unwrap();
    assert_eq!(stored, img_data);

    let info = engine.host_state.blob_store.info(blob_id).unwrap();
    assert_eq!(info.content_type.as_deref(), Some("image/jpeg"));
    assert_eq!(info.file_name.as_deref(), Some("camera.jpg"));

    // Now test 404 failure
    let (port404, _) = serve_raw("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());

    let fetch_id404 = NodeId::generate();
    engine.create_node(fetch_id404, "media/fetch", &[]).unwrap();
    engine
        .set_input_value(
            fetch_id404,
            PinId(1),
            Some(DataValue::String(format!("http://127.0.0.1:{port404}/missing.jpg"))),
        )
        .unwrap();

    let failed_sink2 = sink(&mut engine, fetch_id404, 11);
    engine.trigger_node(fetch_id404, PinId(0));

    run_until(&mut engine, |e| e.executed.iter().any(|t| t.node_id == failed_sink2));

    let err = output(&engine, fetch_id404, 12).expect("Error pin has value");
    assert_eq!(err, DataValue::String("HTTP 404".into()));
}

#[test]
fn media_fetch_node_headers_and_basic_auth() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());

    let (port, req_rx) = serve_raw_full(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK".to_vec());

    let fetch_id = NodeId::generate();
    engine.create_node(fetch_id, "media/fetch", &[]).unwrap();

    engine
        .set_input_value(
            fetch_id,
            PinId(1),
            Some(DataValue::String(format!("http://camuser:campass@127.0.0.1:{port}/feed.jpg"))),
        )
        .unwrap();

    engine
        .set_input_value(
            fetch_id,
            PinId(2),
            Some(DataValue::String("X-Camera-Model: PTZ-9000\nAuthorization: Bearer token123".into())),
        )
        .unwrap();

    let done = sink(&mut engine, fetch_id, 10);
    engine.trigger_node(fetch_id, PinId(0));

    run_until(&mut engine, |e| e.executed.iter().any(|t| t.node_id == done));

    let req_bytes = req_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    let req_str = String::from_utf8_lossy(&req_bytes);

    assert!(req_str.contains("x-camera-model: PTZ-9000"));
    assert!(req_str.contains("authorization: Bearer token123"));
}

#[test]
fn test_plugin_blob_probe_roundtrip() {
    let Some(mut engine) = engine_with("test_plugin", vec![Capability::Storage]) else {
        return;
    };

    let probe = NodeId::generate();
    engine.create_node(probe, "test/blob_probe", &[]).unwrap();

    constant(
        &mut engine,
        DataValue::String("hello from test-plugin blob probe".into()),
        probe,
        1,
    );

    let done = sink(&mut engine, probe, 13);
    engine.trigger_node(probe, PinId(0));
    engine.tick();

    assert!(engine.execution_queue.contains(&(done, PinId(0))));
    let blob_id = output(&engine, probe, 10).expect("Blob output set");
    let size = output(&engine, probe, 11).expect("Size output set");
    let read_text = output(&engine, probe, 12).expect("Read output set");

    let expected_str = "hello from test-plugin blob probe";
    assert_eq!(size, DataValue::Number(NumberValue::Int(expected_str.len() as i64)));
    assert_eq!(read_text, DataValue::String(expected_str.into()));

    let DataValue::String(id_str) = blob_id else {
        panic!("expected String blob id");
    };

    let info = engine.host_state.blob_store.info(&id_str).unwrap();
    assert_eq!(info.size, expected_str.len() as u64);
    assert_eq!(info.content_type.as_deref(), Some("text/plain"));
    assert_eq!(info.file_name.as_deref(), Some("probe.txt"));
}
