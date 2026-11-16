use super::*;
use bindings::ticknode::plugin::host::Host;
use wasmtime::StoreLimitsBuilder;
use wasmtime_wasi::WasiCtxBuilder;

fn state() -> PluginState {
    PluginState {
        settings_error: None,
        wasi: WasiCtxBuilder::new().build(),
        table: ResourceTable::new(),
        limits: StoreLimitsBuilder::new().build(),
        grants: Arc::new(Grants::default()),
        call: None,
        ws: WsHub::new(None),
        http: super::super::http::Tasks::new(None, None),
        options: Arc::new(Mutex::new(OptionStore::default())),
        blob_store: Arc::new(crate::blobs::MemoryBlobStore::new()),
    }
}

fn request(url: &str) -> wit::HttpRequest {
    wit::HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: vec![],
        timeout_ms: 100,
        multipart: vec![],
        save_to_blob: false,
    }
}

#[test]
fn context_services_distinguish_metadata_plugin_tick_and_real_nodes() {
    let mut state = state();
    assert_eq!(state.current_tick(), 0);
    assert_eq!(state.tick_duration_ms(), 0);
    assert!(state.delay(1).is_err());
    state.call = Some(CallCtx {
        node_id: NodeId(uuid::Uuid::nil()),
        host: HostState::default(),
    });
    assert!(state.delay(1).is_err());
    let node = NodeId::generate();
    state.call.as_mut().unwrap().node_id = node;
    state.call.as_mut().unwrap().host.current_tick = 12;
    assert_eq!(state.current_tick(), 12);
    assert_eq!(state.tick_duration_ms(), 100);
    let task = state.delay(4).unwrap();
    assert_eq!(
        state
            .call
            .as_ref()
            .unwrap()
            .host
            .delay_timers_with_tasks()
            .len(),
        1
    );
    state.cancel_task(task);
    assert!(
        state
            .call
            .as_ref()
            .unwrap()
            .host
            .delay_timers_with_tasks()
            .is_empty()
    );
}

#[test]
fn http_capability_validation_happens_before_transport_without_leaking_credentials() {
    let mut state = state();
    for url in [
        "not a URL",
        "file:///tmp/secret",
        "ftp://user:secret@example.com/path",
        "https://user:secret@example.com/token",
    ] {
        let error = state.start_http(request(url)).unwrap_err();
        assert!(!error.contains("secret"), "{error}");
        assert!(!error.contains("/token"), "{error}");
    }
    state.grants = Arc::new(Grants {
        http_domains: vec!["example.com".into()],
        ..Grants::default()
    });
    assert_eq!(
        state
            .start_http(request("https://example.com/"))
            .unwrap_err(),
        "HTTP unavailable: the engine is running without a tokio runtime"
    );
}

#[test]
fn blob_host_limits_are_inclusive_and_reads_have_a_four_mebibyte_cap() {
    let mut state = state();
    let id = state
        .blob_put(
            vec![7; 16 * 1024 * 1024],
            Some("image/png".into()),
            Some("photo.png".into()),
        )
        .unwrap();
    let info = state.blob_info(id.clone()).unwrap();
    assert_eq!(info.size, 16 * 1024 * 1024);
    assert_eq!(info.content_type.as_deref(), Some("image/png"));
    assert_eq!(info.file_name.as_deref(), Some("photo.png"));
    assert!(chrono::DateTime::parse_from_rfc3339(&info.created).is_ok());
    assert_eq!(
        state.blob_read(id.clone(), 0, u32::MAX).unwrap(),
        vec![7; 4 * 1024 * 1024]
    );
    assert_eq!(state.blob_read(id, 16 * 1024 * 1024 - 1, 2).unwrap(), [7]);
    assert_eq!(
        state
            .blob_put(vec![0; 16 * 1024 * 1024 + 1], None, None)
            .unwrap_err(),
        "Data exceeds 16 MiB limit"
    );
}

#[test]
fn option_quota_allows_existing_sources_to_change_at_the_limit() {
    let mut state = state();
    let option = |label: &str| {
        vec![wit::PropertyOption {
            value: "value".into(),
            label: label.into(),
            detail: String::new(),
            icon: None,
        }]
    };
    for i in 0..MAX_OPTION_SOURCES {
        state.set_options(format!("source-{i}"), option("before"));
    }
    assert_eq!(lock(&state.options).sources.len(), MAX_OPTION_SOURCES);
    state.set_options("extra".into(), option("rejected"));
    assert!(!lock(&state.options).sources.contains_key("extra"));
    state.set_options("source-0".into(), option("after"));
    assert_eq!(lock(&state.options).sources["source-0"][0].label, "after");
}
