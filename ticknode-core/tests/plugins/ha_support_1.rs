// --- Home Assistant plugin against a fake Home Assistant WebSocket server ---

const HA_TOKEN: &str = "test-token";

fn ha_services() -> serde_json::Value {
    serde_json::json!({
        "light":{"turn_on":{"name":"Turn on", "target":{"entity":[{"domain":"light"}]}, "fields":{
            "brightness":{"name":"Brightness","required":true,"selector":{"number":{"min":0,"max":255}}},
            "effect":{"selector":{"select":{"options":["none","rainbow"]}}},
            "rgb_color":{"selector":{"color_rgb":null}},
            "transition":{"advanced":true,"selector":{"number":{}}}
        }}},
        "weather":{"get_forecasts":{"name":"Get forecasts", "target":{"entity":[{"domain":"weather"}]}, "fields":{
            "type":{"selector":{"select":{"options":["daily","hourly"]}}}
        },"response":{"optional":false}}},
        "number":{"set_value":{"fields":{
            "value":{"required":true,"selector":{"number":{"unit_of_measurement":"kW"}}}
        }}},
        "homeassistant":{"turn_on":{"name":"Turn on", "target":{}}}
    })
}

/// A minimal Home Assistant: the auth handshake, `subscribe_trigger` / `subscribe_entities` /
/// `unsubscribe_events` / `call_service` as `ha-core-client` uses them, and pushed state changes.
/// Like `echo_server`, it runs in its own runtime on its own thread.
struct FakeHa {
    port: u16,
    /// Message id of each entity's `subscribe_trigger`.
    subs: Arc<std::sync::Mutex<std::collections::HashMap<String, u64>>>,
    /// Current state of each entity (default "off").
    states: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    unsubscribed: Arc<AtomicUsize>,
    connections: Arc<AtomicUsize>,
    /// `call_service` requests as received.
    calls: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// Automation trigger requests, including their message ids and typed configurations.
    trigger_requests: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// What `get_states` answers.
    entity_list: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// Frames to push to every connected client.
    push: tokio::sync::broadcast::Sender<String>,
}

impl FakeHa {
    fn start() -> FakeHa {
        use futures::{SinkExt, StreamExt};
        use serde_json::{Value, json};
        let (push, _) = tokio::sync::broadcast::channel(64);
        let server = FakeHa {
            port: 0,
            subs: Arc::default(),
            states: Arc::default(),
            unsubscribed: Arc::default(),
            connections: Arc::default(),
            calls: Arc::default(),
            entity_list: Arc::default(),
            trigger_requests: Arc::default(),
            push,
        };
        let (subs, states, unsubscribed, connections, calls, push, entity_list, trigger_requests) = (
            server.subs.clone(),
            server.states.clone(),
            server.unsubscribed.clone(),
            server.connections.clone(),
            server.calls.clone(),
            server.push.clone(),
            server.entity_list.clone(),
            server.trigger_requests.clone(),
        );
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    let (subs, states, unsubscribed, connections, calls, entity_list, trigger_requests) = (subs.clone(), states.clone(), unsubscribed.clone(), connections.clone(), calls.clone(), entity_list.clone(), trigger_requests.clone());
                    let mut pushed = push.subscribe();
                    tokio::spawn(async move {
                        use tokio_tungstenite::tungstenite::Message;
                        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else { return };
                        connections.fetch_add(1, Ordering::SeqCst);
                        let _ = ws.send(json!({"type": "auth_required", "ha_version": "2026.10.0"}).to_string().into()).await;
                        let mut authenticated = false;
                        loop {
                            let text = tokio::select! {
                                message = ws.next() => match message {
                                    Some(Ok(Message::Text(text))) => text.to_string(),
                                    Some(Ok(_)) => continue,
                                    _ => break,
                                },
                                frame = pushed.recv() => {
                                    if let Ok(frame) = frame && authenticated {
                                        let _ = ws.send(frame.into()).await;
                                    }
                                    continue;
                                }
                            };
                            let request: Value = serde_json::from_str(&text).unwrap();
                            if !authenticated {
                                let ok = request["type"] == "auth" && request["access_token"] == HA_TOKEN;
                                let reply = if ok { json!({"type": "auth_ok", "ha_version": "2026.10.0"}) } else { json!({"type": "auth_invalid", "message": "Invalid access token"}) };
                                let _ = ws.send(reply.to_string().into()).await;
                                authenticated = ok;
                                continue;
                            }
                            let id = request["id"].clone();
                            let mut replies = vec![json!({"id": id, "type": "result", "success": true, "result": null})];
                            match request["type"].as_str().unwrap_or_default() {
                                "subscribe_trigger" => {
                                    trigger_requests.lock().unwrap().push(request.clone());
                                    let entity = &request["trigger"]["entity_id"];
                                    if let Some(entity) = entity.as_str().or_else(|| entity[0].as_str()) {
                                        subs.lock().unwrap().insert(entity.to_string(), id.as_u64().unwrap());
                                    }
                                    if request["trigger"]["event_type"][0] == "reject" {
                                        replies = vec![json!({"id":id,"type":"result","success":false,"error":{"code":"invalid_format","message":"Trigger rejected by fake HA"}})];
                                    }
                                }
                                "subscribe_events" if request["event_type"] == "state_changed" => {
                                    subs.lock().unwrap().insert("__cache".into(), id.as_u64().unwrap());
                                }
                                "get_config" => {
                                    replies = vec![json!({"id":id,"type":"result","success":true,"result":{"time_zone":"Europe/Kyiv"}})];
                                }
                                "render_template" => {
                                    let value = if request["template"].as_str().unwrap().contains("utcoffset") { json!(7200) }
                                        else if request["template"] == "{{ true }}" { json!(true) } else { json!(false) };
                                    replies.push(json!({"id":id,"type":"event","event":{"result":value,"listeners":{}}}));
                                }
                                "subscribe_entities" => {
                                    let entity = request["entity_ids"][0].as_str().unwrap();
                                    let known = entity_list.lock().unwrap().iter().find(|s| s["entity_id"] == entity).cloned();
                                    let state = states.lock().unwrap().get(entity).cloned()
                                        .or_else(|| known.as_ref()?.get("state")?.as_str().map(str::to_string))
                                        .unwrap_or_else(|| "off".into());
                                    let attrs = known.map(|s| s["attributes"].clone()).unwrap_or_else(|| json!({}));
                                    replies.push(json!({"id": id, "type": "event", "event": {"a": {entity: {"s": state, "a": attrs, "c": "01X", "lc": 1.0, "lu": 1.0}}}}));
                                }
                                "unsubscribe_events" => {
                                    unsubscribed.fetch_add(1, Ordering::SeqCst);
                                }
                                "get_services" => {
                                    replies = vec![json!({"id":id,"type":"result","success":true,"result":ha_services()})];
                                }
                                "get_states" => {
                                    replies = vec![json!({"id": id, "type": "result", "success": true, "result": entity_list.lock().unwrap().clone()})];
                                }
                                "call_service" => {
                                    calls.lock().unwrap().push(request.clone());
                                    if request["return_response"] == true {
                                        replies = vec![json!({"id":id,"type":"result","success":true,"result":{"context":{"id":"call"},"response":{"weather.home":{"forecast":[{"temperature":21}]}}}})];
                                    }
                                    if request["domain"] == "broken" {
                                        replies = vec![json!({"id": id, "type": "result", "success": false, "error": {"code": "not_found", "message": "Service not found."}})];
                                    }
                                }
                                _ => {}
                            }
                            for reply in replies {
                                let _ = ws.send(reply.to_string().into()).await;
                            }
                        }
                    });
                }
            });
        });
        FakeHa {
            port: port_rx.recv().unwrap(),
            ..server
        }
    }

    fn url(&self) -> String {
        format!("ws://127.0.0.1:{}/api/websocket", self.port)
    }

    /// Pushes HA's trigger-variable envelope to a particular subscription.
    fn fire(&self, id: u64, trigger: serde_json::Value) {
        let _ = self.push.send(serde_json::json!({"id":id,"type":"event","event":{"variables":{"trigger":trigger},"context":{"id":"test"}}}).to_string());
    }

    /// Pushes a state change of `entity`, as the entity's `state` trigger subscription reports it.
    fn change(&self, entity: &str, from: &str, to: &str) {
        let mut attributes = serde_json::json!({"friendly_name":"Kitchen", "brightness":120});
        if let Some(state) = self.entity_list.lock().unwrap().iter().find(|s| s["entity_id"] == entity)
            && let Some(attrs) = state["attributes"].as_object()
        {
            attributes.as_object_mut().unwrap().extend(attrs.clone());
        }
        let state = |s: &str| {
            serde_json::json!({
                "entity_id": entity, "state": s, "attributes": attributes,
                "last_changed": "2026-10-30T23:15:42.500000+02:00",
                "last_reported": "2026-10-30T23:15:42.500000+02:00",
                "last_updated": "2026-10-30T23:15:42.500000+02:00",
                "context": {"id": "01X", "parent_id": null, "user_id": null}
            })
        };
        self.states
            .lock()
            .unwrap()
            .insert(entity.to_string(), to.to_string());
        if let Some(id) = self.subs.lock().unwrap().get("__cache").copied() {
            let _ = self.push.send(serde_json::json!({"id":id,"type":"event","event":{"event_type":"state_changed","data":{"entity_id":entity,"old_state":state(from),"new_state":state(to)},"origin":"LOCAL","time_fired":"2026-10-30T10:00:00Z","context":{"id":"cache"}}}).to_string());
        }
        let Some(id) = self.subs.lock().unwrap().get(entity).copied() else {
            return;
        };
        let event = serde_json::json!({"id": id, "type": "event", "event": {"variables": {"trigger": {
            "id": "0", "idx": "0", "alias": null, "platform": "state", "entity_id": entity,
            "from_state": state(from), "to_state": state(to), "for": null, "attribute": null,
        }}, "context": {"id": "01X", "parent_id": null, "user_id": null}}});
        let _ = self.push.send(event.to_string());
    }
}

fn ha_settings(url: &str, token: &str) -> Vec<(String, DataValue)> {
    vec![
        ("url".into(), DataValue::String(url.into())),
        ("token".into(), DataValue::String(token.into())),
    ]
}

fn ha_engine(ha: &FakeHa, token: &str, runtime: &tokio::runtime::Runtime) -> Option<TickEngine> {
    let bytes = plugin_bytes("ha_plugin")?;
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine
        .add_plugin(&bytes, None, |_| {
            vec![Capability::WebSocket {
                allowed_hosts: vec!["127.0.0.1".into()],
            }]
        })
        .unwrap();
    engine
        .enable_plugin("home_assistant", &ha_settings(&ha.url(), token))
        .unwrap();
    Some(engine)
}

fn field_text(value: &DataValue, name: &str) -> Option<String> {
    match value {
        DataValue::Object { fields, .. } => match &fields.get(name)?.value {
            DataValue::String(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Ticks until the sink has executed `count` times in total (starting from `*seen`), returns the new total.
fn count_executions(engine: &mut TickEngine, sink: NodeId, mut seen: usize, until: usize) -> usize {
    for _ in 0..300 {
        engine.tick();
        seen += engine.executed.iter().filter(|t| t.node_id == sink).count();
        if seen >= until {
            return seen;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the sink ran {seen} times, expected {until}");
}

#[test]
fn ha_plugin_declares_its_nodes_and_type() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    let manifest = engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    assert_eq!(manifest.name, "home_assistant");
    assert_eq!(manifest.resources.len(), 3);
    assert_eq!(manifest.resources[0].id, "entities");
    assert_eq!(manifest.resources[0].actions.len(), 2);
    assert_eq!(manifest.resources[1].id, "zones");
    assert_eq!(manifest.resources[1].actions.len(), 2);
    assert_eq!(manifest.resources[2].id, "persons");
    assert_eq!(manifest.resources[2].actions.len(), 1);
    assert!(
        manifest
            .settings
            .iter()
            .any(|s| s.key == "token" && s.is_secret() && s.required)
    );
    assert!(manifest.settings.iter().any(|s| s.key == "url"
        && s.default
            == Some(DataValue::String(
                "ws://homeassistant.local:8123/api/websocket".into()
            ))));
    engine
        .enable_plugin(
            "home_assistant",
            &[("token".into(), DataValue::String("t".into()))],
        )
        .unwrap();
    for node in [
        "ha/action",
        "ha/entity",
        "ha/trigger_time",
        "ha/trigger_time_pattern",
        "ha/trigger_sun",
        "ha/trigger_zone",
        "ha/trigger_event",
        "ha/trigger_homeassistant",
        "ha/trigger_mqtt",
        "ha/trigger_webhook",
        "ha/trigger_calendar",
        "ha/trigger_tag",
        "ha/trigger_conversation",
        "ha/trigger_template",
        "ha/condition_time",
        "ha/condition_sun",
        "ha/condition_zone",
        "ha/condition_template",
    ] {
        let definition = engine.registry.get_definition(node).unwrap();
        assert_eq!(definition.category, "Home Assistant");
        assert!(!definition.help.is_empty());
    }
    assert!(engine.types.get("ha.state").is_some());
    let def = engine.registry.get_definition("ha/entity").unwrap();
    assert_eq!(
        def.properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["entity_id", "value_type", "fire_on"]
    );
    assert_eq!(
        def.properties[0].kind,
        PropertyKind::Dynamic {
            source: "entities".into(),
            plugin: "home_assistant".into()
        }
    );
    let PropertyKind::Select(options) = &def.properties[1].kind else { panic!("expected value type options") };
    assert_eq!(options.iter().take(4).map(|(v, _)| v.as_str()).collect::<Vec<_>>(),
        ["number", "bool", "text", "datetime"]);
    for quantity in ticknode_shared::quantity::catalog() {
        assert!(options.contains(&(quantity.name.clone(), quantity.title.clone())));
    }
    assert!(
        matches!(&def.properties[2].kind, PropertyKind::Select(o) if o.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>() == ["state", "any"])
    );
    let node = NodeId::generate();
    engine.create_node(node, "ha/entity", &[]).unwrap();
    let properties = &engine
        .export_graph()
        .nodes
        .into_iter()
        .find(|n| n.id == node)
        .unwrap()
        .properties;
    assert_eq!(
        properties,
        &vec![
            ("entity_id".into(), "".into()),
            ("value_type".into(), "text".into()),
            ("fire_on".into(), "state".into())
        ]
    );
    // Every message the plugin ships is kept by the host (nothing is dropped as foreign)
    let translations = engine.translations();
    let merged = &translations
        .iter()
        .find(|(locale, _)| locale == "uk")
        .expect("the uk resource")
        .1;
    let shipped = include_str!("../../../ha-plugin/locales/uk.ftl");
    let ids: Vec<_> = shipped
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_lowercase()))
        .filter_map(|l| l.split_once(" =").map(|(id, _)| id))
        .collect();
    assert!(ids.len() >= 40, "{} ids", ids.len());
    for id in ids {
        assert!(
            merged.contains(&format!("{id} =")),
            "{id} is missing from the merged translations"
        );
    }
}
