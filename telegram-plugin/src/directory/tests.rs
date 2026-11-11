use super::*;
use crate::tests::*;
use serde_json::json;

#[test]
fn learns_chat_user_topics_including_reply_creation_and_immediate_renames() {
    let mut dir = Directory::default();
    let mut raw = update(1, "hello");
    raw["message"]["message_thread_id"] = 7.into();
    raw["message"]["reply_to_message"] = message(-100, 7, "");
    raw["message"]["reply_to_message"]["forum_topic_created"] = json!({"name":"Kitchen"});
    dir.learn(&serde_json::from_value(raw.clone()).unwrap(), 1_700_000_000);
    assert_eq!(dir.chats[&-100].name, "Home");
    assert_eq!(dir.users[&17].name, "Ada");
    assert_eq!(dir.topics["-100:7"].name, "Kitchen");
    raw["message"]["chat"]["title"] = "House".into();
    raw["message"]["from"]["first_name"] = "Grace".into();
    raw["message"]["forum_topic_edited"] = json!({"name":"Living room"});
    dir.publish = false;
    dir.learn(&serde_json::from_value(raw).unwrap(), 1_700_000_001);
    assert!(dir.publish);
    assert_eq!(dir.chats[&-100].name, "House");
    assert_eq!(dir.users[&17].name, "Grace");
    assert_eq!(dir.topics["-100:7"].name, "Living room");
}
#[test]
fn first_sighting_per_utc_day_changes_last_seen_without_republishing() {
    let mut dir = Directory::default();
    let now = 1_700_000_000;
    let u = serde_json::from_value(update(1, "x")).unwrap();
    dir.learn(&u, now);
    let mut env = Fake::default();
    dir.publish(&mut env);
    dir.flush(now + 5, &mut env);
    assert!(dir.dirty_at.is_none());
    dir.learn(&u, now + 6);
    assert!(dir.dirty_at.is_none());
    dir.learn(&u, now + 86400);
    assert!(dir.dirty_at.is_some());
    assert!(!dir.publish);
    assert_eq!(dir.users[&17].last_seen, date(now + 86400));
}
#[test]
fn ttl_expiry_runs_at_startup_and_once_daily_zero_never_expires() {
    let mut dir = Directory::default();
    let now = 1_700_000_000;
    dir.learn(&serde_json::from_value(update(1, "x")).unwrap(), now);
    dir.maintain(now + 31 * 86400, 0);
    assert_eq!(dir.chats.len(), 1);
    // The same day has already been checked; a changed TTL is applied on configure to a fresh directory.
    dir.checked_day = None;
    dir.maintain(now + 30 * 86400, 30);
    assert_eq!(dir.chats.len(), 1);
    dir.maintain(now + 31 * 86400, 30);
    assert!(dir.chats.is_empty());
    assert!(dir.users.is_empty());
}
#[test]
fn cap_evicts_least_recently_seen_and_never_exceeds_ten_thousand() {
    let mut dir = Directory::default();
    for id in 0..=CAP as i64 {
        dir.chats.insert(
            id,
            Entry {
                name: id.to_string(),
                username: None,
                detail: String::new(),
                last_seen: date(if id == 7 { 0 } else { 1_700_000_000 }),
            },
        );
    }
    dir.maintain(1_700_000_000, 0);
    assert_eq!(dir.chats.len(), CAP);
    assert!(!dir.chats.contains_key(&7));
}
#[test]
fn debounced_persistence_roundtrip_retains_dates_and_topic_keys() {
    let mut dir = Directory::default();
    let mut raw = update(1, "x");
    raw["message"]["message_thread_id"] = 7.into();
    raw["message"]["forum_topic_created"] = json!({"name":"Kitchen"});
    dir.learn(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    let mut env = Fake::default();
    dir.flush(1_700_000_004, &mut env);
    assert!(env.writes.is_empty());
    dir.flush(1_700_000_005, &mut env);
    assert_eq!(env.writes, vec!["directory.json"]);
    let loaded = Directory::load(&mut env).unwrap();
    assert_eq!(loaded.chats, dir.chats);
    assert_eq!(loaded.users, dir.users);
    assert_eq!(loaded.topics, dir.topics);
    let json = String::from_utf8(env.files["directory.json"].clone()).unwrap();
    assert!(json.contains("2023-11-14"));
}
#[test]
fn corrupt_directory_is_rejected_without_overwriting_it() {
    let mut env = Fake::default();
    env.files
        .insert("directory.json".into(), b"broken".to_vec());
    assert!(Directory::load(&mut env).is_err());
    assert!(env.writes.is_empty());
}
#[test]
fn colliding_topic_labels_keep_both_chat_names_and_raw_identifier_value() {
    let mut dir = Directory::default();
    for (chat, name) in [(-100, "Home"), (-200, "Office")] {
        let mut raw = message(chat, 7, "");
        raw["chat"]["title"] = name.into();
        raw["message_thread_id"] = 7.into();
        raw["forum_topic_created"] = json!({"name":"Kitchen"});
        dir.message(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    }
    let mut env = Fake::default();
    dir.publish(&mut env);
    assert_eq!(dir.topics.len(), 2);
    assert_eq!(env.sources["topics"].len(), 1);
    let opt = &env.sources["topics"][0];
    assert_eq!(opt.value, "7");
    assert!(opt.label.contains("Home › Kitchen"));
    assert!(opt.label.contains("Office › Kitchen"));
    assert_eq!(opt.icon.as_deref(), Some("mdi:forum"));
}
#[test]
fn learns_private_chat_names_and_member_or_inline_callback_users() {
    let mut dir = Directory::default();
    let raw = json!({"update_id":1,"my_chat_member":{"chat":{"id":42,"type":"private","first_name":"Ada","last_name":"Lovelace","username":"ada"},"from":{"id":42,"first_name":"Ada"}}});
    dir.learn(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    assert_eq!(dir.chats[&42].name, "Ada Lovelace");
    let raw = json!({"update_id":2,"callback_query":{"id":"x","from":{"id":51,"first_name":"Grace"},"data":"x"}});
    dir.learn(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    assert_eq!(dir.users[&51].name, "Grace");
}

#[test]
fn historical_topic_creation_reply_cannot_undo_an_existing_topic_rename() {
    let mut dir = Directory::default();
    let mut raw = message(-100, 7, "");
    raw["message_thread_id"] = 7.into();
    raw["forum_topic_edited"] = json!({"name":"New name"});
    dir.message(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    let mut raw = message(-100, 50, "");
    raw["message_thread_id"] = 7.into();
    raw["reply_to_message"] = message(-100, 7, "");
    raw["reply_to_message"]["forum_topic_created"] = json!({"name":"Old name"});
    dir.message(&serde_json::from_value(raw).unwrap(), 1_700_000_001);
    assert_eq!(dir.topics["-100:7"].name, "New name");
}
