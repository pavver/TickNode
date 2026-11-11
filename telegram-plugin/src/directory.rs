//! Persistent ID directory. Topic keys include the chat; wire values remain raw Telegram IDs.
mod learning;
mod options;
#[cfg(test)]
mod tests;

use crate::hub::Env;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CAP: usize = 10_000;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub username: Option<String>,
    pub detail: String,
    /// UTC day since the Unix epoch; serialized as a date for human inspection.
    pub last_seen: chrono::NaiveDate,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Directory {
    pub chats: BTreeMap<i64, Entry>,
    pub users: BTreeMap<i64, Entry>,
    /// JSON cannot encode tuple map keys, so the persisted key is "chat:topic".
    pub topics: BTreeMap<String, Entry>,
    #[serde(skip)]
    pub dirty_at: Option<u64>,
    #[serde(skip)]
    pub publish: bool,
    #[serde(skip)]
    pub checked_day: Option<chrono::NaiveDate>,
}
pub fn date(now: u64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(now.min(i64::MAX as u64) as i64, 0)
        .unwrap_or_default()
        .date_naive()
}
impl Directory {
    pub fn load(env: &mut impl Env) -> Result<Self, String> {
        let mut directory = match env.read("directory.json")? {
            Some(data) => serde_json::from_slice(&data)
                .map_err(|_| "Invalid Telegram directory.json".to_string())?,
            None => Self::default(),
        };
        directory.publish = true;
        Ok(directory)
    }
    pub fn changed(&mut self, now: u64, labels: bool) {
        self.dirty_at.get_or_insert(now);
        self.publish |= labels;
    }
    pub fn maintain(&mut self, now: u64, ttl: u64) {
        let today = date(now);
        if self.checked_day == Some(today) {
            return;
        }
        self.checked_day = Some(today);
        let mut changed = false;
        changed |= prune(&mut self.chats, today, ttl);
        changed |= prune(&mut self.users, today, ttl);
        changed |= prune(&mut self.topics, today, ttl);
        if changed {
            self.changed(now, true);
        }
    }
    pub fn flush(&mut self, now: u64, env: &mut impl Env) {
        if !self.dirty_at.is_some_and(|at| now.saturating_sub(at) >= 5) {
            return;
        }
        if self.save(env).is_err() {
            env.log("Could not persist Telegram directory; retrying in 5 seconds");
            self.dirty_at = Some(now);
        }
    }
    pub fn save(&mut self, env: &mut impl Env) -> Result<(), String> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        env.write("directory.json", &bytes)?;
        self.dirty_at = None;
        Ok(())
    }
}
fn prune<K: Ord + Clone>(map: &mut BTreeMap<K, Entry>, today: chrono::NaiveDate, ttl: u64) -> bool {
    let before = map.len();
    if ttl > 0 {
        map.retain(|_, e| (today - e.last_seen).num_days() <= ttl.min(i64::MAX as u64) as i64);
    }
    if map.len() > CAP {
        let mut oldest: Vec<_> = map
            .iter()
            .map(|(key, entry)| (entry.last_seen, key.clone()))
            .collect();
        oldest.sort();
        for (_, key) in oldest.into_iter().take(map.len() - CAP) {
            map.remove(&key);
        }
    }
    before != map.len()
}
