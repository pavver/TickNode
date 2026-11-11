//! Integration tests for WASM plugins. Requires built components:
//! `cargo build --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p test-plugin -p ha-plugin`.
//! If a binary is missing, the test is skipped with a message (or fails when `TICKNODE_REQUIRE_PLUGINS=1`).
//!
//! Tests are intentionally non-async: synchronous wasmtime-wasi cannot be called inside a tokio task,
//! so the engine, as on the server, runs outside the runtime while a runtime handle is passed for HTTP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use ticknode_core::*;
use ticknode_shared::*;

fn plugin_bytes(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../target/wasm32-wasip2/debug/{name}.wasm"));
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) if std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none() => {
            eprintln!("{} not found - test skipped", path.display());
            None
        }
        Err(e) => panic!("{}: {e}", path.display()),
    }
}

fn engine_with(plugin: &str, capabilities: Vec<Capability>) -> Option<TickEngine> {
    let bytes = plugin_bytes(plugin)?;
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| capabilities).unwrap();
    Some(engine)
}

fn constant(engine: &mut TickEngine, value: DataValue, dst: NodeId, dst_pin: u32) {
    let id = NodeId::generate();
    engine
        .create_node(id, "const", &[("value".into(), value)])
        .unwrap();
    engine
        .connect_data(id, PinId(0), dst, PinId(dst_pin))
        .unwrap();
}

fn sink(engine: &mut TickEngine, src: NodeId, src_pin: u32) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, "test/debug", &[]).unwrap();
    engine
        .connect_exec(src, PinId(src_pin), id, PinId(0))
        .unwrap();
    id
}

fn number(v: i64) -> DataValue {
    DataValue::Number(NumberValue::Int(v))
}

fn output(engine: &TickEngine, node: NodeId, pin: u32) -> Option<DataValue> {
    engine.data_cache.get(&node)?.get(&PinId(pin)).cloned()
}

include!("plugins/builtins_1.rs");
include!("plugins/builtins_2.rs");
include!("plugins/named_types.rs");
include!("plugins/required_inputs.rs");
include!("plugins/settings.rs");
include!("plugins/lifecycle.rs");
include!("plugins/translations.rs");
include!("plugins/websocket.rs");
include!("plugins/ha_support_1.rs");
include!("plugins/ha_support_2.rs");
include!("plugins/catalog_1.rs");
include!("plugins/catalog_2.rs");
include!("plugins/catalog_3.rs");
include!("plugins/quantities.rs");

include!("plugins/properties.rs");

include!("plugins/ha_properties.rs");
include!("plugins/identifiers.rs");
include!("plugins/plugin_http.rs");
