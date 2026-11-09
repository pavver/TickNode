use ticknode_plugin_sdk::*;
use std::cell::RefCell;

/// WebSocket probe state: the single connection (opened from the `ws_url` setting in `configure`)
/// and the graph ids of live `test/ws_probe` nodes (targets of emissions).
#[derive(Default)]
pub(super) struct Ws {
    pub(super) id: Option<WsId>,
    pub(super) probes: Vec<NodeId>,
    pub(super) events: i64,
}

thread_local! {
    pub(super) static WS: RefCell<Ws> = RefCell::new(Ws::default());
}

/// Plugin-level WebSocket check: the connection belongs to the plugin, `Plugin::tick` feeds incoming
/// frames to every probe node as an emission (no `execute` involved).
pub(super) struct WsProbe {
    node: NodeId,
}

impl WsProbe {
    pub(super) fn new(node: NodeId) -> Self {
        if !node.0.is_nil() {
            WS.with(|w| w.borrow_mut().probes.push(node));
        }
        Self { node }
    }
}

impl Drop for WsProbe {
    fn drop(&mut self) {
        WS.with(|w| w.borrow_mut().probes.retain(|n| *n != self.node));
    }
}

impl PluginNode for WsProbe {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "test/ws_probe".into(),
            title: "WebSocket Probe".into(),
            category: "Test".into(),
            description: "Echoes through the plugin's WebSocket".into(),
            help: "The plugin opens a WebSocket to the ws_url setting. A signal on Send sends Text through it. Every received frame is pushed by the plugin to Last (and a signal on Received), the connection state goes to Status.".into(),
            pins: vec![
                exec_in(0, "Send"),
                data_in(1, "Text", DataType::String),
                exec_out(2, "Received"),
                data_out(3, "Last", DataType::String),
                data_out(4, "Status", DataType::String),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let result = match WS.with(|w| w.borrow().id) {
            Some(id) => host::ws_send(id, &inputs.string(PinId(1))).map(|()| "sent".to_string()),
            None => Err("no connection".to_string()),
        };
        vec![Output::Data(PinId(4), result.unwrap_or_else(|e| format!("error: {e}")).into())]
    }
}

pub(super) fn ws_emit(outputs: Vec<Output>) -> Vec<Emission> {
    WS.with(|w| w.borrow().probes.iter().map(|n| Emission::new(*n, outputs.clone())).collect())
}
