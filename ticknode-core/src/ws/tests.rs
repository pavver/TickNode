use super::*;

#[test]
fn core_c_websocket_handles_are_unique_and_accept_the_exact_message_limit() {
    // The current-thread runtime is deliberately not driven: outbound queuing needs no TCP socket.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut hub = WsHub::new(Some(rt.handle().clone()));
    let first = hub.connect("ws://127.0.0.1:1/", &[]).unwrap();
    let second = hub.connect("ws://127.0.0.1:1/", &[]).unwrap();
    hub.close(0);
    assert_eq!(first, 1);
    assert_ne!(first, second);
    assert_eq!(hub.send(first, "x".repeat(MAX_MESSAGE_BYTES)), Ok(()));
    assert!(
        hub.send(first, "x".repeat(MAX_MESSAGE_BYTES + 1))
            .unwrap_err()
            .contains("too large")
    );
    hub.close(first);
    assert!(
        hub.send(first, "closed".into())
            .unwrap_err()
            .contains("Unknown or closed")
    );
    assert_eq!(hub.send(second, "still open".into()), Ok(()));
}

#[test]
fn core_c_websocket_drain_respects_zero_and_exact_event_limits() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut hub = WsHub::new(Some(rt.handle().clone()));
    let id = hub.connect("ws://127.0.0.1:1/", &[]).unwrap();
    for event in [
        WsEvent::Opened(id),
        WsEvent::Message(id, "one".into()),
        WsEvent::Message(id, "two".into()),
        WsEvent::Closed(id, "done".into()),
    ] {
        hub.inbox_tx.try_send(event).unwrap();
    }
    assert_eq!(hub.drain(0), []);
    assert_eq!(hub.drain(1), [WsEvent::Opened(id)]);
    assert_eq!(
        hub.drain(2),
        [
            WsEvent::Message(id, "one".into()),
            WsEvent::Message(id, "two".into())
        ]
    );
    assert_eq!(hub.drain(1), [WsEvent::Closed(id, "done".into())]);
    assert!(hub.drain(1).is_empty());
    assert!(hub.send(id, "closed".into()).is_err());
}

#[test]
fn core_c_websocket_transport_preserves_default_tcp_coalescing() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/", listener.local_addr().unwrap());
        let request = build_request(&url, &[]).unwrap();
        let (stream, server) = tokio::join!(open_stream(request), async {
            let (stream, _) = listener.accept().await.unwrap();
            tokio_tungstenite::accept_async(stream).await.unwrap()
        });
        let stream = stream.unwrap();
        let tokio_tungstenite::MaybeTlsStream::Plain(socket) = stream.get_ref() else {
            panic!("expected plain TCP")
        };
        assert!(
            !socket.nodelay().unwrap(),
            "plugin transports keep the default Nagle policy"
        );
        drop(server);
    });
}
