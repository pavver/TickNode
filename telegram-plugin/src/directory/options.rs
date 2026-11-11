//! A context-free identifier label combines colliding topic IDs from different chats.
use super::*;
use ticknode_plugin_sdk::PropertyOption;

fn option(value: String, name: &str, detail: String, icon: &str) -> PropertyOption {
    PropertyOption {
        value,
        label: clip(name),
        detail: clip(&detail),
        icon: Some(icon.into()),
    }
}
fn clip(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        if result.len() + ch.len_utf8() > 500 {
            break;
        }
        result.push(ch);
    }
    result
}
impl Directory {
    pub fn publish(&mut self, env: &mut impl Env) {
        if !std::mem::take(&mut self.publish) {
            return;
        }
        for (source, entries) in [("chats", &self.chats), ("users", &self.users)] {
            let options: Vec<_> = entries
                .iter()
                .map(|(id, entry)| {
                    let detail = [
                        entry
                            .username
                            .as_ref()
                            .map(|s| format!("@{s}"))
                            .unwrap_or_default(),
                        entry.detail.clone(),
                    ]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                    let icon = if source == "users" || entry.detail == "private" {
                        "mdi:account"
                    } else {
                        "mdi:account-group"
                    };
                    option(id.to_string(), &entry.name, detail, icon)
                })
                .collect();
            env.options(source, &options);
        }
        let mut topics: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        for (key, entry) in &self.topics {
            let Some((chat, topic)) = key
                .split_once(':')
                .and_then(|(c, t)| Some((c.parse::<i64>().ok()?, t.parse::<i64>().ok()?)))
            else {
                continue;
            };
            let chat_name = self
                .chats
                .get(&chat)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| chat.to_string());
            topics
                .entry(topic)
                .or_default()
                .push(format!("{chat_name} › {}", entry.name));
        }
        let options: Vec<_> = topics
            .into_iter()
            .map(|(id, names)| {
                let label = names.join(" / ");
                option(id.to_string(), &label, label.clone(), "mdi:forum")
            })
            .collect();
        env.options("topics", &options);
    }
}
