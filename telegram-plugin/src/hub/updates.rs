//! Update ingestion, directory learning, callback acknowledgements and per-node queues.
use super::*;
use crate::model::Update;
impl Hub {
    pub(super) fn updates(&mut self, value: Value, now: u64, env: &mut impl Env) {
        let Some(updates) = value.as_array() else {
            env.log("Invalid Telegram getUpdates result");
            return;
        };
        for raw in updates {
            let Some(id) = raw["update_id"].as_i64() else {
                continue;
            };
            if id < self.offset {
                continue;
            }
            self.offset = id.saturating_add(1);
            self.offset_dirty = true;
            let Ok(update) = serde_json::from_value::<Update>(raw.clone()) else {
                env.log("Unsupported or malformed Telegram update skipped");
                continue;
            };
            debug_assert_eq!(update.update_id, id);
            // Even ignored chats advance the offset. Learning is independent of trigger filters.
            self.directory.learn(&update, now);
            if let Some(message) = update.message() {
                self.remember(message);
            }
            let allowed = update.chat().is_some_and(|chat| self.permits(chat))
                || (update.chat().is_none() && self.allowed.is_empty());
            let mut toast = None;
            if allowed {
                for watch in self.watches.values_mut() {
                    if let Some(outputs) = watch
                        .filter
                        .outputs(&update, self.username.as_deref().unwrap_or(""))
                    {
                        if update.callback_query.is_some() && toast.is_none() {
                            toast = Some(watch.filter.toast().to_string());
                        }
                        if watch.queue.len() >= 128 {
                            watch.queue.pop_front();
                            watch.overflow = true;
                        }
                        watch.queue.push_back(outputs);
                    }
                }
            }
            if let Some(callback) = &update.callback_query {
                if self.aux.len() < 256 {
                    self.aux.push_back((
                        Request::Answer,
                        json!({"callback_query_id":callback.id,"text":toast.unwrap_or_default()}),
                    ));
                } else {
                    env.log("Telegram callback acknowledgement queue full");
                }
            }
        }
    }
}
