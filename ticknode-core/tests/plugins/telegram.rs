// Local sockets are required; run outside restricted sandboxes.
// Build the separate test component with telegram-plugin's test-api feature; see its README.
struct TelegramFake {
    url: String,
    received: std::sync::mpsc::Receiver<(String, serde_json::Value)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl TelegramFake {
    fn start() -> Self {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (send, received) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let delivered = Arc::new(AtomicBool::new(false));
        let thread = std::thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("Fake Telegram accept: {error}"),
                };
                let send = send.clone();
                let delivered = delivered.clone();
                std::thread::spawn(move || {
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    let mut header = Vec::new();
                    let mut byte = [0];
                    while !header.ends_with(b"\r\n\r\n") {
                        if socket.read(&mut byte).unwrap_or(0) == 0 {
                            return;
                        }
                        header.push(byte[0]);
                        assert!(header.len() < 16_384);
                    }
                    let header = String::from_utf8(header).unwrap();
                    let method = header
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .to_string();
                    let length: usize = header
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                        .unwrap()
                        .1
                        .trim()
                        .parse()
                        .unwrap();
                    let mut body = vec![0; length];
                    socket.read_exact(&mut body).unwrap();
                    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    let _ = send.send((method.clone(), body));
                    let message = serde_json::json!({"message_id":8,"date":1700000000,"chat":{"id":42,"type":"private","first_name":"Ada"},"text":"/status","from":{"id":42,"first_name":"Ada","is_bot":false}});
                    let result = match method.as_str() {
                        "getMe" => {
                            serde_json::json!({"id":1,"is_bot":true,"username":"TickBot","first_name":"Tick"})
                        }
                        "getUpdates" if !delivered.swap(true, Ordering::SeqCst) => {
                            serde_json::json!([{"update_id":20,"message":message}])
                        }
                        "getUpdates" => serde_json::json!([]),
                        "sendMessage" => {
                            let mut result = message;
                            result["message_id"] = 9.into();
                            result["text"] = "Status: OK".into();
                            result
                        }
                        "getChat" => {
                            serde_json::json!({"id":42,"type":"private","first_name":"Ada"})
                        }
                        _ => panic!("Unexpected Telegram method {method}"),
                    };
                    let text = serde_json::json!({"ok":true,"result":result}).to_string();
                    let _ = write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        text.len(),
                        text
                    );
                });
            }
        });
        Self {
            url,
            received,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for TelegramFake {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn telegram_plugin_message_to_send_end_to_end() {
    let Some(bytes) = plugin_bytes("telegram_plugin_test") else {
        return;
    };
    let fake = TelegramFake::start();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let storage = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine
        .add_plugin(&bytes, Some(storage.path()), |_| {
            vec![
                Capability::Http {
                    allowed_domains: vec!["127.0.0.1".into()],
                },
                Capability::Storage,
            ]
        })
        .unwrap();
    engine
        .enable_plugin(
            "telegram",
            &[
                ("token".into(), "fake-token".into()),
                ("allowed_chats".into(), "42".into()),
                ("test_api_url".into(), fake.url.clone().into()),
            ],
        )
        .unwrap();
    assert!(engine.plugin_settings_error("telegram").is_none());
    let receive = NodeId::generate();
    let send = NodeId::generate();
    engine
        .create_node(receive, "telegram/message", &[])
        .unwrap();
    engine.create_node(send, "telegram/send", &[]).unwrap();
    engine
        .connect_exec(receive, PinId(0), send, PinId(0))
        .unwrap();
    engine
        .connect_data(receive, PinId(4), send, PinId(1))
        .unwrap();
    engine
        .set_input_value(send, PinId(3), Some("Status: OK".into()))
        .unwrap();
    run_until(&mut engine, |e| output(e, send, 14) == Some(9i64.into()));
    assert_eq!(output(&engine, receive, 4), Some(42i64.into()));
    assert!(
        engine.node_errors().is_empty(),
        "{:?}",
        engine.node_errors()
    );
    let requests: Vec<_> = fake.received.try_iter().collect();
    let sent = requests
        .iter()
        .find(|(method, _)| method == "sendMessage")
        .unwrap();
    assert_eq!(sent.1["chat_id"], 42);
    assert_eq!(sent.1["text"], "Status: OK");
    assert_eq!(
        std::fs::read(storage.path().join("telegram/offset.json")).unwrap(),
        b"21"
    );
    // Reload from the real WASI preopen: the next poll must confirm the persisted offset.
    engine.disable_plugin("telegram").unwrap();
    engine.host_state.logs.clear();
    engine
        .enable_plugin(
            "telegram",
            &[
                ("token".into(), "fake-token".into()),
                ("test_api_url".into(), fake.url.clone().into()),
            ],
        )
        .unwrap();
    run_until(&mut engine, |e| {
        e.host_state
            .logs
            .iter()
            .any(|(_, line)| line.contains("authenticated"))
    });
    let mut confirmed = false;
    run_until(&mut engine, |_| {
        confirmed |= fake
            .received
            .try_iter()
            .any(|(method, body)| method == "getUpdates" && body["offset"] == 21);
        confirmed
    });
}

#[test]
fn telegram_plugin_catalog_registers_translations_identifiers_and_property_input_contracts() {
    let Some(bytes) = plugin_bytes("telegram_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    let manifest = engine
        .add_plugin(&bytes, None, |manifest| {
            manifest.requested_capabilities.clone()
        })
        .unwrap();
    assert_eq!(manifest.name, "telegram");
    assert_eq!(
        manifest.requested_capabilities,
        vec![
            Capability::Http {
                allowed_domains: vec!["api.telegram.org".into()]
            },
            Capability::Storage,
        ]
    );
    engine.enable_plugin("telegram", &[]).unwrap();
    assert_eq!(
        engine.plugin_settings_error("telegram").unwrap().key,
        "telegram-error-token"
    );
    for (name, source) in [
        ("chat_id", Some("chats")),
        ("user_id", Some("users")),
        ("message_id", None),
        ("topic_id", Some("topics")),
    ] {
        let ty = engine.types.get(&format!("telegram.{name}")).unwrap();
        assert_eq!(ty.plugin.as_deref(), Some("telegram"));
        assert!(matches!(&ty.kind, TypeKind::Identifier { labels } if labels.as_deref() == source));
    }
    for locale in ["en", "uk"] {
        let translations = engine
            .translations()
            .into_iter()
            .find(|(code, _)| code == locale)
            .unwrap()
            .1;
        for kind in ["message", "command", "callback", "send", "edit", "delete"] {
            assert!(translations.contains(&format!("node-telegram-{kind}-help =")));
        }
    }
    let send = NodeId::generate();
    engine
        .create_node(send, "telegram/send", &[("chat".into(), "42".into())])
        .unwrap();
    let definition = engine.node_definition(send).unwrap();
    assert_eq!(definition.cost, 50);
    assert!(
        !definition
            .pins
            .iter()
            .any(|pin| pin.direction == PinDirection::Input && pin.id == PinId(1))
    );
    assert!(
        definition
            .pins
            .iter()
            .any(|pin| pin.id == PinId(3) && pin.required)
    );
    engine.set_node_property(send, "chat", "".into()).unwrap();
    assert!(
        engine
            .node_definition(send)
            .unwrap()
            .pins
            .iter()
            .any(|pin| pin.id == PinId(1))
    );
    assert!(
        engine
            .set_node_property(send, "buttons", "Bad markup".into())
            .is_err()
    );
    let receive = NodeId::generate();
    engine
        .create_node(receive, "telegram/message", &[])
        .unwrap();
    assert_eq!(engine.node_definition(receive).unwrap().cost, 2);
}
