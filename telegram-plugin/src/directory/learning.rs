//! Learn names immediately, while touching last_seen only on the first sighting of each UTC day.
use super::*;
use crate::model::{Chat, Message, Update, User};

impl Directory {
    fn put<K: Ord + Clone>(map: &mut BTreeMap<K, Entry>, key: K, mut entry: Entry) -> (bool, bool) {
        let labels = map.get(&key).is_none_or(|old| {
            old.name != entry.name || old.username != entry.username || old.detail != entry.detail
        });
        if let Some(old) = map.get(&key) {
            entry.last_seen = old.last_seen.max(entry.last_seen);
        }
        if map.get(&key) == Some(&entry) {
            return (false, false);
        }
        map.insert(key, entry);
        prune(map, chrono::NaiveDate::MIN, 0);
        (true, labels)
    }
    pub fn chat(&mut self, chat: &Chat, now: u64) {
        let name = chat
            .title
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                full_name(
                    chat.first_name.as_deref().unwrap_or(""),
                    chat.last_name.as_deref(),
                )
            });
        let name = if name.is_empty() {
            chat.username
                .clone()
                .map(|s| format!("@{s}"))
                .unwrap_or_default()
        } else {
            name
        };
        let name = if name.is_empty() {
            let Some(known) = self.chats.get(&chat.id) else {
                return;
            };
            known.name.clone()
        } else {
            name
        };
        let (changed, labels) = Self::put(
            &mut self.chats,
            chat.id,
            Entry {
                name,
                username: chat.username.clone(),
                detail: chat.kind.clone(),
                last_seen: date(now),
            },
        );
        if changed {
            self.changed(now, labels);
        }
    }
    pub fn user(&mut self, user: &User, now: u64) {
        let (changed, labels) = Self::put(
            &mut self.users,
            user.id,
            Entry {
                name: full_name(&user.first_name, user.last_name.as_deref()),
                username: user.username.clone(),
                detail: String::new(),
                last_seen: date(now),
            },
        );
        if changed {
            self.changed(now, labels);
        }
    }
    pub fn message(&mut self, message: &Message, now: u64) {
        // A quoted message is a historical snapshot: it must never undo a later rename.
        if let Some(reply) = &message.reply_to_message {
            if !self.chats.contains_key(&reply.chat.id) {
                self.chat(&reply.chat, now);
            }
            if let Some(user) = &reply.from
                && !self.users.contains_key(&user.id)
            {
                self.user(user, now);
            }
            let topic = reply
                .message_thread_id
                .or_else(|| reply.forum_topic_created.as_ref().map(|_| reply.message_id));
            if let Some(topic) = topic
                && !self
                    .topics
                    .contains_key(&format!("{}:{topic}", reply.chat.id))
            {
                self.topic(reply, now);
            }
        }
        self.chat(&message.chat, now);
        if let Some(user) = &message.from {
            self.user(user, now);
        }
        self.topic(message, now);
    }
    fn topic(&mut self, message: &Message, now: u64) {
        let Some(topic) = message.message_thread_id.or_else(|| {
            message
                .forum_topic_created
                .as_ref()
                .map(|_| message.message_id)
        }) else {
            return;
        };
        let key = format!("{}:{topic}", message.chat.id);
        let name = message
            .forum_topic_edited
            .as_ref()
            .and_then(|t| t.name.clone())
            .or_else(|| message.forum_topic_created.as_ref()?.name.clone())
            .or_else(|| self.topics.get(&key).map(|e| e.name.clone()));
        let Some(name) = name else {
            return;
        };
        let (changed, labels) = Self::put(
            &mut self.topics,
            key,
            Entry {
                name,
                username: None,
                detail: message.chat.id.to_string(),
                last_seen: date(now),
            },
        );
        if changed {
            self.changed(now, labels);
        }
    }
    pub fn learn(&mut self, update: &Update, now: u64) {
        if let Some(message) = update.message() {
            self.message(message, now);
        }
        if let Some(callback) = &update.callback_query {
            self.user(&callback.from, now);
        }
        if let Some(member) = &update.my_chat_member {
            self.chat(&member.chat, now);
            self.user(&member.from, now);
        }
    }
}
fn full_name(first: &str, last: Option<&str>) -> String {
    format!("{first} {}", last.unwrap_or("")).trim().to_string()
}
