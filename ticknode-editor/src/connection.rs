//! WebSocket to ticknode-server: bidirectional postcard messages with automatic reconnection.

use std::time::Duration;

use leptos::prelude::*;
use ticknode_shared::ServerMessage;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{BinaryType, MessageEvent, WebSocket};

use crate::state::{ConnStatus, Editor, LogKind};

const RECONNECT_DELAY: Duration = Duration::from_millis(1500);

/// Relative `ws` endpoint next to the current page — works both directly and behind an HA ingress prefix.
fn ws_url() -> Option<String> {
    let href = window().location().href().ok()?;
    let url = web_sys::Url::new_with_base("ws", &href).ok()?;
    url.set_protocol(if url.protocol() == "https:" {
        "wss:"
    } else {
        "ws:"
    });
    url.set_search("");
    url.set_hash("");
    Some(url.href())
}

pub fn connect(editor: Editor) {
    editor.status.set(ConnStatus::Connecting);
    let Some(ws) = ws_url().and_then(|url| WebSocket::new(&url).ok()) else {
        schedule_reconnect(editor);
        return;
    };
    ws.set_binary_type(BinaryType::Arraybuffer);

    let on_open = Closure::<dyn FnMut()>::new(move || {
        editor.status.set(ConnStatus::Online);
        editor.log(LogKind::System, "[UI] Connected to server");
    });
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        match postcard::from_bytes::<ServerMessage>(&bytes) {
            Ok(msg) => editor.apply(msg),
            Err(e) => editor.log(
                LogKind::Error,
                format!("[ERROR] Invalid server message: {e}"),
            ),
        }
    });
    let on_close = Closure::<dyn FnMut()>::new(move || {
        let was_online = editor.status.get_untracked() == ConnStatus::Online;
        editor.socket.set_value(None);
        editor.status.set(ConnStatus::Offline);
        if was_online {
            editor.log(
                LogKind::Error,
                "[ERROR] Connection to server lost, reconnecting...",
            );
        }
        schedule_reconnect(editor);
    });

    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    // Handlers live for the lifetime of the socket; each reconnect leaks three small closures.
    on_open.forget();
    on_message.forget();
    on_close.forget();

    editor.socket.set_value(Some(ws));
}

fn schedule_reconnect(editor: Editor) {
    set_timeout(move || connect(editor), RECONNECT_DELAY);
}
