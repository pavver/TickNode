use super::*;
use crate::{TickEngine, host::HostState};
use ticknode_shared::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn request() -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: "http://127.0.0.1:1/".into(),
        headers: vec![],
        body: vec![],
        timeout: std::time::Duration::from_secs(70),
    }
}

#[test]
fn plugin_owned_task_limit_cancellation_and_runtime_errors_need_no_network() {
    let rt = runtime(); // Never driven: no HTTP future can open a socket.
    let mut tasks = Tasks::new(Some(rt.handle().clone()));
    let ids: Vec<_> = (0..8).map(|_| tasks.start(request()).unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]));
    assert!(tasks.start(request()).unwrap_err().contains("maximum 8"));
    assert!(!tasks.cancel(1));
    assert!(tasks.cancel(ids[0]));
    assert!(!tasks.cancel(ids[0]));
    assert!(tasks.start(request()).is_ok());
    assert!(tasks.drain(1000).is_empty());
    drop(tasks);
    assert!(
        Tasks::default()
            .start(request())
            .unwrap_err()
            .contains("without a tokio runtime")
    );
    assert!(crate::host::MAX_HTTP_TIMEOUT >= std::time::Duration::from_secs(65));
}

#[test]
fn completed_plugin_tasks_deliver_success_and_errors_once_in_start_order() {
    let rt = runtime();
    let mut tasks = Tasks::default();
    tasks.pending.insert(
        42,
        rt.spawn(async {
            Ok(TaskOutput::Http(crate::host::HttpResponse {
                status: 200,
                headers: vec![("x-test".into(), "yes".into())],
                body: b"reply".to_vec(),
            }))
        }),
    );
    tasks
        .pending
        .insert(43, rt.spawn(async { Err("Timed out".into()) }));
    rt.block_on(async {
        tokio::task::yield_now().await;
    });
    assert!(tasks.drain(0).is_empty());
    let events = tasks.drain(1);
    let [wit::WsEvent::HttpDone(done)] = events.as_slice() else {
        panic!("expected HTTP event")
    };
    assert_eq!(done.id, 42);
    let Ok(wit::TaskOutput::Http(response)) = &done.result else {
        panic!("expected response")
    };
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"reply");
    let events = tasks.drain(1000);
    let [wit::WsEvent::HttpDone(done)] = events.as_slice() else {
        panic!("expected HTTP error")
    };
    assert_eq!(done.id, 43);
    assert_eq!(done.result.as_ref().unwrap_err(), "Timed out");
    assert!(tasks.drain(1000).is_empty());
}

fn engine(rt: &tokio::runtime::Runtime) -> Option<TickEngine> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(path) else {
        assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none());
        return None;
    };
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(rt.handle().clone());
    engine
        .load_plugin(&bytes, None, |_| {
            vec![Capability::Http {
                allowed_domains: vec!["127.0.0.1".into()],
            }]
        })
        .unwrap();
    Some(engine)
}

#[test]
fn plugin_http_permissions_and_owners_are_enforced_without_driving_transport() {
    use super::super::{bindings::ticknode::plugin::host::Host, host::CallCtx};
    let rt = runtime();
    let Some(engine) = engine(&rt) else { return };
    let mut plugin = super::super::lock(&engine.plugins["test_plugin"].runtime);
    let (store, _) = plugin.api().unwrap();
    let state = store.data_mut();
    let req = |url: &str| wit::HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: vec![],
        timeout_ms: 70_000,
    };
    assert!(
        state
            .start_http(req("http://evil.example/"))
            .unwrap_err()
            .contains("not allowed")
    );
    assert!(
        state
            .start_http(req("file:///tmp/test"))
            .unwrap_err()
            .contains("Unsupported scheme")
    );
    let task = state.start_http(req("http://127.0.0.1:1/")).unwrap();
    let owner = NodeId::generate();
    let mut host = HostState::default();
    host.runtime = Some(rt.handle().clone());
    state.call = Some(CallCtx {
        node_id: owner,
        host,
    });
    assert!(
        !state.cancel_task(task),
        "a node cannot cancel a plugin-owned task"
    );
    let node_task = state.start_http(req("http://127.0.0.1:1/")).unwrap();
    assert!(
        state
            .call
            .as_ref()
            .unwrap()
            .host
            .active_tasks
            .contains_key(&node_task)
    );
    let context = state.call.take().unwrap();
    assert!(
        !state.cancel_task(node_task),
        "the plugin cannot cancel a node-owned task"
    );
    assert!(state.cancel_task(task));
    assert!(!state.cancel_task(task));
    drop(context); // All futures are aborted before this runtime is ever driven.
}

struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for NotifyDrop {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

#[test]
fn plugin_tasks_abort_on_disable_crash_and_engine_drop() {
    for action in ["disable", "crash", "drop"] {
        let rt = runtime();
        let Some(mut engine) = engine(&rt) else {
            return;
        };
        let (dropped, wait_drop) = tokio::sync::oneshot::channel();
        let (started, wait_start) = tokio::sync::oneshot::channel();
        let task = rt.spawn(async move {
            let _guard = NotifyDrop(Some(dropped));
            let _ = started.send(());
            futures::future::pending::<TaskResult>().await
        });
        rt.block_on(wait_start).unwrap();
        {
            let mut plugin = super::super::lock(&engine.plugins["test_plugin"].runtime);
            plugin
                .instance
                .as_mut()
                .unwrap()
                .0
                .data_mut()
                .http
                .pending
                .insert(100, task);
        }
        match action {
            "disable" => {
                engine.disable_plugin("test_plugin").unwrap();
            }
            "crash" => {
                let crash = NodeId::generate();
                engine.create_node(crash, "test/crash", &[]).unwrap();
                engine.trigger_node(crash, PinId(0));
                engine.tick();
            }
            _ => drop(engine),
        }
        rt.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(1), wait_drop)
                .await
                .unwrap()
                .unwrap();
        });
    }
}

#[test]
fn plugin_tick_delivers_http_completion_through_wit_sdk_and_emissions_without_network() {
    let rt = runtime();
    let Some(mut engine) = engine(&rt) else {
        return;
    };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/http_probe", &[]).unwrap();
    let response = || {
        Ok(TaskOutput::Http(crate::host::HttpResponse {
            status: 200,
            headers: vec![],
            body: b"plugin reply".to_vec(),
        }))
    };
    let task = rt.spawn(async move { response() });
    rt.block_on(async {
        tokio::task::yield_now().await;
    });
    {
        let mut plugin = super::super::lock(&engine.plugins["test_plugin"].runtime);
        plugin
            .instance
            .as_mut()
            .unwrap()
            .0
            .data_mut()
            .http
            .pending
            .insert(42, task);
    }
    engine.tick();
    assert_eq!(engine.data_cache[&probe][&PinId(1)], "plugin reply".into());
    assert_eq!(engine.data_cache[&probe][&PinId(3)], 1i64.into());
    assert!(
        engine
            .executed
            .iter()
            .any(|t| t.node_id == probe && t.fired.contains(&PinId(4)))
    );
    assert!(engine.host_state.active_tasks.is_empty());
    engine.tick();
    assert_eq!(engine.data_cache[&probe][&PinId(3)], 1i64.into());
    // A finished result from a discarded instance must never reach its replacement.
    let task = rt.spawn(async { Err("stale result".into()) });
    rt.block_on(async {
        tokio::task::yield_now().await;
    });
    {
        let mut plugin = super::super::lock(&engine.plugins["test_plugin"].runtime);
        plugin
            .instance
            .as_mut()
            .unwrap()
            .0
            .data_mut()
            .http
            .pending
            .insert(43, task);
        plugin.discard();
    }
    engine.tick();
    assert_eq!(engine.data_cache[&probe][&PinId(3)], 0i64.into());
    assert_eq!(engine.data_cache[&probe][&PinId(1)], "".into());
}
