use crate::bindings::exports::ticknode::plugin::nodes as wit_nodes;
use crate::bindings::ticknode::plugin::types as wit;
use crate::convert::*;
use crate::*;
use std::cell::RefCell;
use std::marker::PhantomData;

fn node_id_from_wit((hi, lo): (u64, u64)) -> NodeId {
    NodeId(uuid_from_halves(hi, lo))
}

fn node_id_to_wit(id: NodeId) -> (u64, u64) {
    id.0.as_u64_pair()
}

fn uuid_from_halves(hi: u64, lo: u64) -> ticknode_shared::Uuid {
    ticknode_shared::Uuid::from_u64_pair(hi, lo)
}

/// Exports the plugin as a WASM component. Invoked once in a plugin crate.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        type __TickNodeExport = $crate::Exporter<$plugin>;
        $crate::bindings::__export_ticknode_plugin!(__TickNodeExport with_types_in $crate::bindings);
    };
}

// --- WIT export implementation ---

#[doc(hidden)]
pub struct Exporter<P>(PhantomData<P>);

#[doc(hidden)]
pub struct NodeHandle(RefCell<Box<dyn PluginNode>>);

impl<P: Plugin> wit_nodes::Guest for Exporter<P> {
    type Node = NodeHandle;

    fn manifest() -> wit::PluginManifest {
        let m = P::manifest();
        wit::PluginManifest {
            name: m.name,
            version: m.version,
            author: m.author,
            capabilities: m
                .requested_capabilities
                .into_iter()
                .map(|c| match c {
                    Capability::Http { allowed_domains } => wit::Capability::Http(allowed_domains),
                    Capability::Storage => wit::Capability::Storage,
                    Capability::WebSocket { allowed_hosts } => {
                        wit::Capability::Websocket(allowed_hosts)
                    }
                })
                .collect(),
            settings: m.settings.into_iter().map(to_wit_setting).collect(),
        }
    }

    fn configure(settings: Vec<wit::Property>) -> Result<(), wit::UiError> {
        let settings: Vec<(String, DataValue)> = settings
            .into_iter()
            .filter_map(|p| Some((p.name, decode(&p.value)?)))
            .collect();
        P::configure(&settings).map_err(|e| wit::UiError {
            key: e.key,
            args: e.args,
            fallback: e.fallback,
        })
    }

    fn translations() -> Vec<(String, String)> {
        P::TRANSLATIONS
            .iter()
            .map(|(locale, ftl)| ((*locale).to_string(), (*ftl).to_string()))
            .collect()
    }

    fn definitions() -> Vec<wit::NodeDefinition> {
        P::NODE_TYPES
            .iter()
            .filter_map(|t| P::create(t, NodeId(ticknode_shared::Uuid::nil())))
            .map(|n| to_wit_definition(n.definition(), n.polls()))
            .collect()
    }

    fn types() -> Vec<wit::TypeDefinition> {
        P::types().into_iter().map(to_wit_type_definition).collect()
    }

    fn create_node(type_name: String, node: wit::NodeId) -> Result<wit_nodes::Node, String> {
        P::create(&type_name, node_id_from_wit(node))
            .map(|node| wit_nodes::Node::new(NodeHandle(RefCell::new(node))))
            .ok_or_else(|| format!("Unknown node type: {type_name}"))
    }

    fn plugin_tick(events: Vec<wit::WsEvent>) -> Vec<wit::Emission> {
        let events = events
            .into_iter()
            .map(|e| match e {
                wit::WsEvent::Opened(id) => WsEvent::Opened(id),
                wit::WsEvent::Message((id, text)) => WsEvent::Message(id, text),
                wit::WsEvent::Closed((id, reason)) => WsEvent::Closed(id, reason),
            })
            .collect();
        P::tick(events)
            .into_iter()
            .map(|e| wit::Emission {
                node: node_id_to_wit(e.node),
                outputs: e.outputs.into_iter().map(to_wit_output).collect(),
            })
            .collect()
    }
}

impl wit_nodes::GuestNode for NodeHandle {
    fn definition(&self) -> wit::NodeDefinition {
        let node = self.0.borrow();
        to_wit_definition(node.definition(), node.polls())
    }

    fn properties(&self) -> Vec<wit::Property> {
        self.0
            .borrow()
            .properties()
            .into_iter()
            .map(|(name, value)| wit::Property {
                name,
                value: encode(&value),
            })
            .collect()
    }

    fn set_property(&self, name: String, value: wit::Value) {
        if let Some(value) = decode(&value) {
            self.0.borrow_mut().set_property(&name, value);
        }
    }

    fn execute(&self, trigger: wit::Trigger, inputs: Vec<(u32, wit::Value)>) -> Vec<wit::Output> {
        let trigger = from_wit_trigger(trigger);
        let inputs = inputs
            .into_iter()
            .filter_map(|(pin, v)| Some((PinId(pin), decode(&v)?)))
            .collect();
        self.0
            .borrow_mut()
            .execute(trigger, &inputs)
            .into_iter()
            .map(to_wit_output)
            .collect()
    }

    fn poll(&self, tick: u64) -> Option<u32> {
        self.0.borrow_mut().poll(tick).map(|p| p.0)
    }
}

fn from_wit_trigger(trigger: wit::Trigger) -> Trigger {
    match trigger {
        wit::Trigger::Pin(pin) => Trigger::Pin(PinId(pin)),
        wit::Trigger::Task(done) => Trigger::Task {
            id: done.id,
            result: done.result.map(|output| match output {
                wit::TaskOutput::Delay => TaskOutput::Delay,
                wit::TaskOutput::Http(r) => TaskOutput::Http(HttpResponse {
                    status: r.status,
                    headers: r.headers,
                    body: r.body,
                }),
            }),
        },
    }
}
