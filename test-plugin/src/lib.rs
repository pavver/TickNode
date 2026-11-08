//! Plugin for host integration tests (`ticknode-core/tests/plugins.rs`).

use ticknode_plugin_sdk::*;

mod settings;
mod websocket;
mod execution;
mod typed;

use settings::{CONFIG, Greeter, Picker, publish_options};
use websocket::{WS, WsProbe, ws_emit};
use execution::{Counter, Ticker, Crash, Storage, ClearProbe};
use typed::{POINT, MODE, PointSource, BadSource, PointSink};

struct TestPlugin;

impl Plugin for TestPlugin {
    const NODE_TYPES: &'static [&'static str] =
        &["test/clear", "test/bad_source", "test/counter", "test/crash", "test/greeter", "test/picker", "test/point_sink", "test/point_source", "test/storage", "test/ticker", "test/ws_probe"];
    /// `de` has a syntax error (rejected by the host); `uk` contains an id the plugin may not define (dropped).
    const TRANSLATIONS: &'static [(&'static str, &'static str)] =
        &[("uk", include_str!("../locales/uk.ftl")), ("de", "node-test-counter-title = Zähler\nthis is not fluent"), ("zz", include_str!("../locales/zz.ftl"))];

    fn configure(settings: &[(String, DataValue)]) -> Result<(), UiError> {
        let token = setting_str(settings, "token").unwrap_or_default().to_string();
        // Reconnect on every configure (also after the instance is recreated following a crash)
        let ws_result = WS.with(|w| {
            let mut w = w.borrow_mut();
            if let Some(id) = w.id.take() {
                host::ws_close(id);
            }
            match setting_str(settings, "ws_url") {
                Some(url) => host::ws_connect(url, &[("x-test".to_string(), "1".to_string())])
                    .map(|id| w.id = Some(id))
                    .map_err(|e| UiError::new("test-plugin-error-ws", format!("WebSocket: {e}")).arg("reason", e)),
                None => Ok(()),
            }
        });
        CONFIG.with(|c| {
            let mut c = c.borrow_mut();
            c.greeting = setting_str(settings, "greeting").unwrap_or_default().to_string();
            publish_options(&c.greeting);
            c.mode = setting_str(settings, "mode").unwrap_or("quiet").to_string();
            c.token = token.clone();
        });
        match token.as_str() {
            "" => Err(UiError::new("test-plugin-error-no-token", "Token is not set")),
            "bad" => Err(UiError::new("test-plugin-error-bad-token", "Invalid token").arg("length", token.len())),
            // A key outside the plugin namespace must be dropped by the host
            "foreign" => Err(UiError::new("node-math-add-title", "Foreign key")),
            "plain" => Err("Plain error".into()),
            _ => ws_result,
        }
    }

    fn tick(events: Vec<WsEvent>) -> Vec<Emission> {
        WS.with(|w| w.borrow_mut().events += events.len() as i64);
        let mut emissions = Vec::new();
        for event in events {
            match event {
                WsEvent::Opened(_) => emissions.extend(ws_emit(vec![Output::Data(PinId(4), "open".into())])),
                WsEvent::Message(_, text) => emissions.extend(ws_emit(vec![Output::Data(PinId(3), text.into()), Output::Exec(PinId(2))])),
                WsEvent::Closed(id, reason) => {
                    WS.with(|w| {
                        let mut w = w.borrow_mut();
                        if w.id == Some(id) {
                            w.id = None;
                        }
                    });
                    emissions.extend(ws_emit(vec![Output::Data(PinId(4), format!("closed: {reason}").into())]));
                }
            }
        }
        emissions
    }

    fn types() -> Vec<TypeDefinition> {
        vec![
            TypeDefinition::new_struct(POINT, "Point", vec![
                FieldDefinition::new("x", "X", DataType::Number),
                FieldDefinition::new("y", "Y", DataType::Number),
                FieldDefinition::new("label", "Label", DataType::String).optional(),
            ]).described("A point on a plane").colored("#e0a030"),
            TypeDefinition::new_enum(MODE, "Mode", &[("auto", "Auto"), ("manual", "Manual")]),
        ]
    }

    fn manifest() -> PluginManifest {
        PluginManifest::new("test_plugin", "0.1.0", "TickNode")
            .capability(Capability::Storage)
            .capability(Capability::WebSocket { allowed_hosts: vec!["*".into()] })
            .setting(SettingDefinition::text("greeting", "Greeting").described("What the Greeter node emits").default_value("Hello"))
            .setting(SettingDefinition::secret("token", "Token").described("The plugin rejects an empty token or \"bad\"").required())
            .setting(SettingDefinition::select("mode", "Mode", &[("quiet", "Quiet"), ("loud", "Loud")]).default_value("quiet"))
            .setting(SettingDefinition::text("ws_url", "WebSocket URL").described("Opened by the plugin for the ws_probe node"))
    }

    fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> {
        Some(match type_name {
            "test/clear" => Box::new(ClearProbe),
            "test/counter" => Box::new(Counter { step: 1, count: 0 }),
            "test/crash" => Box::new(Crash),
            "test/greeter" => Box::new(Greeter),
            "test/picker" => Box::new(Picker { choice: String::new(), mode: "a".into() }),
            "test/bad_source" => Box::new(BadSource),
            "test/point_source" => Box::new(PointSource { x: 0.0, y: 0.0 }),
            "test/point_sink" => Box::new(PointSink),
            "test/storage" => Box::new(Storage),
            "test/ticker" => Box::new(Ticker { polls: 0 }),
            "test/ws_probe" => Box::new(WsProbe::new(node)),
            _ => return None,
        })
    }
}

export_plugin!(TestPlugin);
