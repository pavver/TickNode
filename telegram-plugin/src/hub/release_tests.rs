use super::*;
use crate::schema;
use crate::tests::*;

#[test]
fn topic_cache_keeps_exact_capacity_then_evicts_only_the_oldest_message() {
    let (mut hub, _) = authenticated();
    for id in 1..=10_000 {
        let mut value = message(-100, id, "topic");
        value["message_thread_id"] = (id + 100).into();
        hub.remember(&serde_json::from_value(value).unwrap());
    }
    assert_eq!(hub.reply_topic(-100, 1), Some(101));
    let mut value = message(-100, 10_001, "new");
    value["message_thread_id"] = 10_101.into();
    hub.remember(&serde_json::from_value(value).unwrap());
    assert_eq!(hub.reply_topic(-100, 1), None);
    assert_eq!(hub.reply_topic(-100, 2), Some(102));
    assert_eq!(hub.reply_topic(-100, 10_001), Some(10_101));
}

#[test]
fn replacement_watch_survives_old_subscription_removal_and_overflow_logs_once() {
    let (mut hub, mut env) = authenticated();
    let node = NodeId::generate();
    let filter = || Filter::new("message", schema::defaults("message")).unwrap();
    let old = hub.register(node, filter());
    let current = hub.register(node, filter());
    assert_ne!(old, current);
    hub.remove(node, old);
    assert!(hub.tick(1_700_000_000, vec![], &mut env).is_empty());
    assert!(!env.logs.iter().any(|text| text.contains("queue full")));
    let values: Vec<_> = (1..=129).map(|id| update(id, "message")).collect();
    let emissions = hub.tick(1_700_000_001, vec![done(2, json!(values))], &mut env);
    assert_eq!(emissions.len(), 1);
    assert!(emissions[0].outputs.contains(&Output::Exec(PinId(0))));
    let overflow_count = |env: &Fake| {
        env.logs
            .iter()
            .filter(|text| text.contains("queue full"))
            .count()
    };
    assert_eq!(overflow_count(&env), 1);
    assert_eq!(hub.tick(1_700_000_002, vec![], &mut env).len(), 1);
    assert_eq!(overflow_count(&env), 1);
}

#[test]
fn invalid_settings_stop_requests_and_successful_reconfigure_clears_authentication_error() {
    let (mut hub, mut env) = authenticated();
    hub.unauthorized();
    assert!(hub.ready().is_err());
    env.errors.clear();
    hub.configure(
        &[
            ("token".into(), "new-token".into()),
            ("directory_ttl_days".into(), 0i64.into()),
        ],
        1_700_000_001,
        &mut env,
    )
    .unwrap();
    hub.tick(1_700_000_001, vec![], &mut env);
    assert!(env.errors.is_empty());
    assert!(
        hub.configure(
            &[
                ("token".into(), "new-token".into()),
                ("allowed_chats".into(), "bad".into())
            ],
            1_700_000_002,
            &mut env
        )
        .is_err()
    );
    let count = env.requests.len();
    hub.tick(1_700_000_003, vec![], &mut env);
    assert_eq!(env.requests.len(), count);
}

#[test]
fn failed_authentication_transport_uses_one_then_two_second_backoff() {
    let (mut hub, mut env) = authenticated();
    env.logs.clear();
    env.fail_http = true;
    hub.configure(&[("token".into(), "secret".into())], 100, &mut env)
        .unwrap();
    let failures = |env: &Fake| {
        env.logs
            .iter()
            .filter(|s| s.contains("Could not start"))
            .count()
    };
    assert_eq!(failures(&env), 1);
    hub.tick(101, vec![], &mut env);
    assert_eq!(failures(&env), 2);
    hub.tick(102, vec![], &mut env);
    assert_eq!(failures(&env), 2);
    hub.tick(103, vec![], &mut env);
    assert_eq!(failures(&env), 3);
}

#[test]
fn known_and_forbidden_lookup_targets_do_not_consume_auxiliary_slots() {
    let (mut hub, mut env) = authenticated();
    hub.directory.chat(
        &serde_json::from_value(json!({"id":-100,"type":"supergroup","title":"Known"})).unwrap(),
        1_700_000_000,
    );
    hub.lookup(-100, 1_700_000_001);
    hub.tick(1_700_000_001, vec![], &mut env);
    assert_eq!(env.requests.len(), 2);
    for chat in 1..=300 {
        hub.lookup(chat, 1_700_000_001);
    }
    assert_eq!(hub.aux.len(), 256);
    hub.tick(1_700_000_002, vec![], &mut env);
    assert_eq!(hub.pending.len(), 6);
}

#[test]
fn successful_poll_resets_the_retry_sequence() {
    let (mut hub, mut env) = authenticated();
    let mut now = 1_700_000_001;
    for reset in [false, true] {
        if reset {
            let id = env.requests.last().unwrap().0;
            hub.tick(now, vec![done(id, json!([]))], &mut env);
            now += 1;
        }
        for delay in [1, 2] {
            let id = env.requests.last().unwrap().0;
            let count = env.requests.len();
            hub.tick(
                now,
                vec![PluginEvent::HttpDone {
                    id,
                    result: response(
                        500,
                        json!({"ok":false,"error_code":500,"description":"failed"}),
                    ),
                }],
                &mut env,
            );
            hub.tick(now + delay - 1, vec![], &mut env);
            assert_eq!(env.requests.len(), count);
            hub.tick(now + delay, vec![], &mut env);
            assert_eq!(env.requests.len(), count + 1);
            now += delay + 1;
        }
    }
}

#[test]
fn zero_server_retry_after_waits_at_least_one_second() {
    let (mut hub, mut env) = authenticated();
    let now = 1_700_000_001;
    let id = env.requests.last().unwrap().0;
    let count = env.requests.len();
    hub.tick(
        now,
        vec![PluginEvent::HttpDone {
            id,
            result: response(
                429,
                json!({"ok":false,"error_code":429,"parameters":{"retry_after":0}}),
            ),
        }],
        &mut env,
    );
    hub.tick(now, vec![], &mut env);
    assert_eq!(env.requests.len(), count);
    hub.tick(now + 1, vec![], &mut env);
    assert_eq!(env.requests.len(), count + 1);
}
