//! Sans-IO bot session, driven once per engine tick with an injected UTC clock.
mod polling;
#[cfg(test)]
mod tests;
mod updates;

use crate::{api, directory::Directory, model::Message, triggers::Filter, validation};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use ticknode_plugin_sdk::*;

pub trait Env {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String>;
    fn cancel(&mut self, id: TaskId);
    fn delay(&mut self, seconds: u64) -> Result<TaskId, String>;
    fn log(&mut self, text: &str);
    fn settings_error(&mut self, error: &UiError);
    fn options(&mut self, source: &str, options: &[PropertyOption]);
    fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Atomic replacement (temporary file + rename).
    fn write(&mut self, name: &str, data: &[u8]) -> Result<(), String>;
}
pub struct Watch {
    pub serial: u64,
    pub filter: Filter,
    pub queue: VecDeque<Vec<Output>>,
    pub overflow: bool,
}
#[derive(Clone)]
enum Request {
    Me,
    Poll,
    Chat(i64),
    Answer,
}
#[derive(Default)]
pub struct Hub {
    pub token: String,
    pub base: String,
    pub username: Option<String>,
    pub allowed: BTreeSet<i64>,
    pub directory: Directory,
    pub generation: u64,
    ttl: u64,
    pub offset: i64,
    offset_dirty: bool,
    stopped: bool,
    authentication_error: bool,
    next: u64,
    backoff: u64,
    pending: BTreeMap<TaskId, Request>,
    aux: VecDeque<(Request, Value)>,
    lookup_days: BTreeMap<i64, u64>,
    pub watches: BTreeMap<NodeId, Watch>,
    serial: u64,
    messages: BTreeMap<(i64, i64), i64>,
    message_order: VecDeque<(i64, i64)>,
}
impl Hub {
    pub fn configure(
        &mut self,
        settings: &[(String, DataValue)],
        now: u64,
        env: &mut impl Env,
    ) -> Result<(), UiError> {
        for id in self.pending.keys() {
            env.cancel(*id);
        }
        self.pending.clear();
        self.aux.clear();
        self.token.clear();
        self.username = None;
        self.generation += 1;
        self.stopped = true;
        self.authentication_error = false;
        self.next = now;
        self.backoff = 1;
        self.messages.clear();
        self.message_order.clear();
        for watch in self.watches.values_mut() {
            watch.queue.clear();
            watch.overflow = false;
        }
        let token = setting_str(settings, "token").unwrap_or("").trim();
        if token.is_empty() {
            return Err(validation::error(
                "token",
                "The Telegram bot token is required",
            ));
        }
        let allowed = setting_str(settings, "allowed_chats").unwrap_or("");
        self.allowed = if allowed.trim().is_empty() {
            BTreeSet::new()
        } else {
            allowed
                .split(',')
                .map(|s| s.trim().parse::<i64>())
                .collect::<Result<_, _>>()
                .map_err(|_| {
                    validation::error(
                        "allowed-chats",
                        "Allowed chats must be comma-separated signed decimal IDs",
                    )
                })?
        };
        let ttl = setting_number(settings, "directory_ttl_days").unwrap_or(30.0);
        if !ttl.is_finite() || ttl < 0.0 || ttl.fract() != 0.0 {
            return Err(validation::error(
                "ttl",
                "Directory retention must be a nonnegative whole number of days",
            ));
        }
        self.ttl = ttl as u64;
        if self.directory.dirty_at.is_some() {
            self.directory
                .save(env)
                .map_err(|s| validation::error("storage", &s))?;
        }
        self.directory = Directory::load(env).map_err(|s| validation::error("storage", &s))?;
        self.directory.maintain(now, self.ttl);
        self.directory.publish(env);
        if self.offset_dirty {
            env.write("offset.json", self.offset.to_string().as_bytes())
                .map_err(|s| validation::error("storage", &s))?;
        }
        self.offset = match env
            .read("offset.json")
            .map_err(|s| validation::error("storage", &s))?
        {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| validation::error("storage", "Invalid Telegram offset.json"))?,
            None => 0,
        };
        self.offset_dirty = false;
        self.lookup_days = match env
            .read("lookups.json")
            .map_err(|s| validation::error("storage", &s))?
        {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| validation::error("storage", "Invalid Telegram lookups.json"))?,
            None => BTreeMap::new(),
        };
        self.base = api::BASE.into();
        #[cfg(feature = "test-api")]
        if let Some(base) = setting_str(settings, "test_api_url") {
            self.base = base.trim_end_matches('/').into();
        }
        self.token = token.into();
        self.stopped = false;
        self.start(Request::Me, json!({}), env, now);
        Ok(())
    }
    pub fn permits(&self, chat: i64) -> bool {
        self.allowed.is_empty() || self.allowed.contains(&chat)
    }
    pub fn ready(&self) -> Result<(), UiError> {
        if self.stopped || self.token.is_empty() {
            return Err(validation::error(
                "unauthorized",
                "Telegram rejected the bot token; update the plugin settings",
            ));
        }
        if self.username.is_none() {
            return Err(validation::error(
                "connecting",
                "Telegram is verifying the bot token; try again shortly",
            ));
        }
        Ok(())
    }
    pub fn register(&mut self, node: NodeId, filter: Filter) -> u64 {
        self.serial += 1;
        self.watches.insert(
            node,
            Watch {
                serial: self.serial,
                filter,
                queue: VecDeque::new(),
                overflow: false,
            },
        );
        self.serial
    }
    pub fn remove(&mut self, node: NodeId, serial: u64) {
        if self.watches.get(&node).is_some_and(|w| w.serial == serial) {
            self.watches.remove(&node);
        }
    }
    pub fn remember(&mut self, message: &Message) {
        if let Some(topic) = message.message_thread_id {
            let key = (message.chat.id, message.message_id);
            if self.messages.insert(key, topic).is_none() {
                self.message_order.push_back(key);
            }
            if self.message_order.len() > 10_000
                && let Some(old) = self.message_order.pop_front()
            {
                self.messages.remove(&old);
            }
        }
        if let Some(reply) = &message.reply_to_message {
            if let Some(topic) = reply.message_thread_id.or(message.message_thread_id) {
                let key = (message.chat.id, reply.message_id);
                if self.messages.insert(key, topic).is_none() {
                    self.message_order.push_back(key);
                }
                if self.message_order.len() > 10_000
                    && let Some(old) = self.message_order.pop_front()
                {
                    self.messages.remove(&old);
                }
            }
        }
    }
    pub fn reply_topic(&self, chat: i64, message: i64) -> Option<i64> {
        self.messages.get(&(chat, message)).copied().or_else(|| {
            self.directory
                .topics
                .contains_key(&format!("{chat}:{message}"))
                .then_some(message)
        })
    }
    pub fn lookup(&mut self, chat: i64, now: u64) {
        if !self.permits(chat)
            || self.directory.chats.contains_key(&chat)
            || self.lookup_days.get(&chat) == Some(&(now / 86400))
        {
            return;
        }
        if self.aux.len() < 256
            && !self
                .aux
                .iter()
                .any(|(r, _)| matches!(r, Request::Chat(id) if *id == chat))
        {
            self.aux
                .push_back((Request::Chat(chat), json!({"chat_id":chat})));
        }
    }
    pub fn unauthorized(&mut self) {
        self.stopped = true;
        self.username = None;
        self.authentication_error = true;
    }
}
