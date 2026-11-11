//! Plugin-level state: the single Home Assistant connection (a sans-IO `Session`), the per-node
//! subscriptions and the in-flight requests. It talks to the outside world only through [`Env`], and the
//! clock is passed in, so the whole logic runs in tests on the host target.

mod collection;
mod connection;
mod env;
mod options;
mod requests;
mod subscriptions;
mod templates;

pub use env::Env;

use crate::queue::ChangeQueue;
use crate::triggers::TriggerConfig;
use ha_core_client::{
    ConnectionState, PendingSubscription, Reply, Session, State, StateChange, TemplateUpdate,
    TriggerFired,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};
use ticknode_plugin_sdk::{NodeId, PropertyOption, WsId};

/// How long to wait before subscribing again after a subscription failed (for example, the entity does not exist).
const RESUBSCRIBE_DELAY: Duration = Duration::from_secs(30);
/// How often the entity list for the picker is refreshed while connected.
const ENTITIES_REFRESH: Duration = Duration::from_secs(5 * 60);
/// Wait before asking for the entity list again after a failed request.
const ENTITIES_RETRY: Duration = Duration::from_secs(30);

struct Link {
    url: String,
    session: Session,
    ws: Option<WsId>,
}

enum Sub<T> {
    /// Not subscribed (no entity yet, or a new one was just set).
    Idle,
    Pending(PendingSubscription<T>),
    Active(ha_core_client::SessionEvents<T>),
    /// The subscription failed or ended; try again at `retry_at`.
    Failed {
        retry_at: Instant,
    },
}

struct Watch<T, C> {
    /// Distinguishes a node from an older one with the same id, so a late `Drop` cannot remove the new one.
    serial: u64,
    config: C,
    queue: ChangeQueue<T>,
    sub: Sub<T>,
    /// Errors delivered from the node's poll, where host logging has the node context.
    errors: Vec<String>,
    /// Initial subscription or restored connection needs a fresh baseline for entity data nodes.
    snapshot_due: bool,
}

struct InFlight {
    node: NodeId,
    reply: Reply<Box<serde_json::value::RawValue>>,
}

struct TemplateCall {
    node: NodeId,
    serial: u64,
    sub: Sub<TemplateUpdate<serde_json::Value>>,
    deadline: Instant,
}

struct EntityWatch {
    reply: Option<Reply<State>>,
    value_type: crate::entity::ValueType,
    fire_on: crate::entity::FireOn,
    unit_warning: bool,
}

#[derive(Default)]
pub struct Hub {
    pub live: crate::live::Live,
    conditions: HashMap<NodeId, u64>,
    templates: Vec<TemplateCall>,
    link: Option<Link>,
    watches: HashMap<NodeId, Watch<StateChange, String>>,
    triggers: HashMap<NodeId, Watch<TriggerFired, Option<TriggerConfig>>>,
    requests: Vec<InFlight>,
    next_serial: u64,
    /// The last connection state that was logged.
    logged: Option<ConnectionState>,
    /// The in-flight `get_states` request that feeds the entity picker.
    entities_reply: Option<Reply<Vec<State>>>,
    /// When the entity list is asked for next. `None`: as soon as the connection is up.
    entities_due: Option<Instant>,
    services_reply: Option<Reply<serde_json::Value>>,
    services_due: Option<Instant>,
    pub services: BTreeMap<String, crate::actions::Service>,
    entity_options: Vec<PropertyOption>,
    domain_sources: BTreeSet<String>,
    action_targets: HashMap<NodeId, String>,
    /// Entity data nodes share the watch lifecycle and additionally read a baseline after (re)subscribe.
    entities: HashMap<NodeId, EntityWatch>,
    /// Entities targeted by a sent action whose resulting state has not arrived yet (see `requests.rs`).
    commanded: HashMap<String, requests::Commanded>,
}

const NOT_CONFIGURED: &str =
    "Home Assistant is not configured (set the URL and the access token in the plugin settings)";

#[cfg(test)]
mod tests;
