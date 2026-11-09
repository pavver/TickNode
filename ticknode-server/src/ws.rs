use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use ticknode_shared::{ClientMessage, ServerMessage, SheetId, UiMessage};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::actor::{EngineHandle, encode};

pub async fn handler(ws: WebSocketUpgrade, State(engine): State<EngineHandle>) -> Response {
    ws.on_upgrade(move |socket| client(socket, engine))
}

async fn client(socket: WebSocket, engine: EngineHandle) {
    let (mut sink, mut stream) = socket.split();
    // Subscribe before taking the snapshot: an event in between may arrive twice, but will not be lost.
    let mut events = engine.subscribe();
    let (reply_tx, mut reply_rx) = mpsc::unbounded_channel::<ServerMessage>();

    let Some(snapshot) = engine.snapshot(None).await else { return };
    let mut open_sheet: Option<SheetId> = match &snapshot {
        ServerMessage::Init { open_sheet, .. } => *open_sheet,
        _ => None,
    };
    if sink.send(Message::Binary(encode(&snapshot))).await.is_err() {
        return;
    }
    debug!("Editor connected");

    loop {
        tokio::select! {
            // Drain broadcast events first: the initiator's `Created` reply references nodes from `NodeAdded`
            // messages that the actor broadcast right before it.
            biased;
            event = events.recv() => match event {
                Ok((sheet_tag, bytes)) => {
                    if sheet_tag.is_none() || sheet_tag == open_sheet {
                        // Only a sheet-tagged `Init` (variant 0, the first postcard byte) sends the
                        // session home after its sheet was deleted; skip decoding everything else.
                        if sheet_tag.is_some() && bytes.first() == Some(&0) {
                            if let Ok(ServerMessage::Init { open_sheet: None, .. }) = postcard::from_bytes::<ServerMessage>(&bytes) {
                                open_sheet = None;
                            }
                        }
                        if sink.send(Message::Binary(bytes)).await.is_err() { break }
                    }
                }
                // Client fell behind: drop the connection so it receives a fresh snapshot on reconnect.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Editor fell behind, dropping the connection");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            },
            Some(reply) = reply_rx.recv() => {
                if sink.send(Message::Binary(encode(&reply))).await.is_err() { break }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Binary(bytes))) => match postcard::from_bytes::<ClientMessage>(&bytes) {
                    Ok(ClientMessage::OpenSheet(target)) => {
                        match engine.open_sheet(target).await {
                            Ok(init_msg) => {
                                open_sheet = target;
                                if sink.send(Message::Binary(encode(&init_msg))).await.is_err() { break }
                            }
                            Err(e) => {
                                if sink.send(Message::Binary(encode(&ServerMessage::Error(e)))).await.is_err() { break }
                            }
                        }
                    }
                    Ok(msg) => engine.send(msg, open_sheet, reply_tx.clone()),
                    Err(e) => { let _ = reply_tx.send(ServerMessage::Error(UiMessage::new("error-bad-message", format!("Invalid message: {e}")).arg("error", e))); }
                },
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    debug!("Editor disconnected");
}
