use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use ticknode_shared::{ClientMessage, ServerMessage};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

use crate::actor::{EngineHandle, encode};

pub async fn handler(ws: WebSocketUpgrade, State(engine): State<EngineHandle>) -> Response {
    ws.on_upgrade(move |socket| client(socket, engine))
}

async fn client(socket: WebSocket, engine: EngineHandle) {
    let (mut sink, mut stream) = socket.split();
    // Subscribe before taking the snapshot: an event in between may arrive twice, but will not be lost. The editor handles events idempotently.
    let mut events = engine.subscribe();
    let (reply_tx, mut reply_rx) = mpsc::unbounded_channel::<ServerMessage>();

    let Some(snapshot) = engine.snapshot().await else { return };
    if sink.send(Message::Binary(encode(&snapshot))).await.is_err() {
        return;
    }
    debug!("Редактор підключився");

    loop {
        tokio::select! {
            // Drain broadcast events first: the initiator's `Created` reply references nodes from `NodeAdded`
            // messages that the actor broadcast right before it.
            biased;
            event = events.recv() => match event {
                Ok(bytes) => if sink.send(Message::Binary(bytes)).await.is_err() { break },
                // Client fell behind: drop the connection so it receives a fresh snapshot on reconnect.
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Редактор відстав, розриваємо з'єднання");
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            Some(reply) = reply_rx.recv() => {
                if sink.send(Message::Binary(encode(&reply))).await.is_err() { break }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Binary(bytes))) => match postcard::from_bytes::<ClientMessage>(&bytes) {
                    Ok(msg) => engine.send(msg, reply_tx.clone()),
                    Err(e) => { let _ = reply_tx.send(ServerMessage::Error(format!("Некоректне повідомлення: {}", e))); }
                },
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    debug!("Редактор відключився");
}
