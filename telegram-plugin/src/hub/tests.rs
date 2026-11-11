use super::*;
use crate::tests::*;

#[test]
fn getme_authenticates_then_exactly_one_long_poll_with_required_updates() {
    let (mut hub, mut env) = authenticated();
    assert_eq!(hub.username.as_deref(), Some("TickBot"));
    assert!(env.logs.iter().any(|line| line.contains("@TickBot")));
    assert_eq!(env.requests.len(), 2);
    let req = &env.requests[1].1;
    assert!(req.url.ends_with("/getUpdates"));
    assert_eq!(req.timeout_ms, 60_000);
    assert_eq!(body(req)["timeout"], 50);
    assert_eq!(
        body(req)["allowed_updates"],
        json!([
            "message",
            "edited_message",
            "callback_query",
            "my_chat_member"
        ])
    );
    for t in 1_700_000_001..1_700_000_100 {
        hub.tick(t, vec![], &mut env);
    }
    assert_eq!(env.requests.len(), 2);
}
#[test]
fn offset_persists_before_next_poll_and_survives_restart_without_replay() {
    let (mut hub, mut env) = authenticated();
    hub.tick(
        1_700_000_001,
        vec![done(2, json!([update(123, "hello")]))],
        &mut env,
    );
    assert_eq!(env.files["offset.json"], b"124");
    assert_eq!(body(&env.requests.last().unwrap().1)["offset"], 124);
    let mut restarted = Hub::default();
    restarted
        .configure(
            &[("token".into(), "secret".into())],
            1_700_000_002,
            &mut env,
        )
        .unwrap();
    assert_eq!(restarted.offset, 124);
}
#[test]
fn offset_storage_failure_blocks_acknowledging_poll() {
    let (mut hub, mut env) = authenticated();
    env.fail_storage = true;
    hub.tick(
        1_700_000_001,
        vec![done(2, json!([update(12, "x")]))],
        &mut env,
    );
    assert_eq!(env.requests.len(), 2);
    env.fail_storage = false;
    hub.tick(1_700_000_002, vec![], &mut env);
    assert_eq!(body(&env.requests[2].1)["offset"], 13);
}
#[test]
fn poll_errors_back_off_to_thirty_seconds_and_409_is_clear() {
    let (mut hub, mut env) = authenticated();
    let mut now = 1_700_000_001;
    for delay in [1, 2, 4, 8, 16, 30, 30] {
        let id = env.requests.last().unwrap().0;
        let count = env.requests.len();
        hub.tick(
            now,
            vec![PluginEvent::HttpDone {
                id,
                result: response(
                    409,
                    json!({"ok":false,"error_code":409,"description":"Conflict"}),
                ),
            }],
            &mut env,
        );
        hub.tick(now + delay - 1, vec![], &mut env);
        assert_eq!(env.requests.len(), count);
        now += delay;
        hub.tick(now, vec![], &mut env);
        assert_eq!(env.requests.len(), count + 1);
    }
    assert!(
        env.logs
            .iter()
            .any(|s| s.contains("webhook") && s.contains("other getUpdates instances"))
    );
}
#[test]
fn unauthorized_becomes_settings_error_and_stops_polling_until_configured() {
    let (mut hub, mut env) = authenticated();
    hub.tick(
        1_700_000_001,
        vec![PluginEvent::HttpDone {
            id: 2,
            result: response(401, json!({"ok":false,"error_code":401})),
        }],
        &mut env,
    );
    assert_eq!(env.errors[0].key, "telegram-error-unauthorized");
    hub.tick(1_800_000_000, vec![], &mut env);
    assert_eq!(env.requests.len(), 2);
}
#[test]
fn auth_transport_failure_retries_without_exposing_token() {
    let (mut hub, mut env) = (Hub::default(), Fake::default());
    hub.configure(&[("token".into(), "secret".into())], 10, &mut env)
        .unwrap();
    hub.tick(
        11,
        vec![PluginEvent::HttpDone {
            id: 1,
            result: Err("https://api.telegram.org/botsecret/getMe".into()),
        }],
        &mut env,
    );
    assert!(!env.logs.iter().any(|s| s.contains("secret")));
    hub.tick(12, vec![], &mut env);
    assert_eq!(env.requests.len(), 2);
}
#[test]
fn allowlist_suppresses_emissions_but_advances_offset_and_learns_directory() {
    let (mut hub, mut env) = authenticated();
    hub.allowed.insert(42);
    hub.register(
        NodeId::generate(),
        Filter::new("message", crate::schema::defaults("message")).unwrap(),
    );
    let emissions = hub.tick(
        1_700_000_001,
        vec![done(2, json!([update(1, "ignored")]))],
        &mut env,
    );
    assert!(emissions.is_empty());
    assert_eq!(hub.offset, 2);
    assert!(hub.directory.chats.contains_key(&-100));
}
#[test]
fn bounded_per_node_queues_emit_at_most_one_pulse_per_tick() {
    let (mut hub, mut env) = authenticated();
    hub.register(
        NodeId::generate(),
        Filter::new("message", crate::schema::defaults("message")).unwrap(),
    );
    let updates: Vec<_> = (0..150).map(|id| update(id, "queued")).collect();
    let emissions = hub.tick(1_700_000_001, vec![done(2, json!(updates))], &mut env);
    assert_eq!(emissions.len(), 1);
    assert_eq!(hub.watches.values().next().unwrap().queue.len(), 127);
    assert!(env.logs.iter().any(|s| s.contains("queue full")));
    assert_eq!(hub.tick(1_700_000_002, vec![], &mut env).len(), 1);
}
#[test]
fn callback_auto_answer_and_toast_use_plugin_owned_http() {
    let (mut hub, mut env) = authenticated();
    let mut props = crate::schema::defaults("callback");
    props.insert("pattern".into(), "status".into());
    props.insert("toast".into(), "Refreshing".into());
    hub.register(NodeId::generate(), Filter::new("callback", props).unwrap());
    let callback = json!({"update_id":1,"callback_query":{"id":"q","from":{"id":17,"first_name":"Ada"},"data":"status","message":message(-100,1,"x")}});
    assert_eq!(
        hub.tick(1_700_000_001, vec![done(2, json!([callback]))], &mut env)
            .len(),
        1
    );
    let req = env
        .requests
        .iter()
        .find(|(_, r)| r.url.ends_with("answerCallbackQuery"))
        .unwrap();
    assert_eq!(
        body(&req.1),
        json!({"callback_query_id":"q","text":"Refreshing"})
    );
}
#[test]
fn getchat_only_for_unknown_send_targets_once_per_day_across_restart() {
    let (mut hub, mut env) = authenticated();
    hub.lookup(42, 1_700_000_001);
    hub.lookup(42, 1_700_000_001);
    hub.tick(1_700_000_001, vec![], &mut env);
    assert_eq!(
        env.requests
            .iter()
            .filter(|(_, r)| r.url.ends_with("getChat"))
            .count(),
        1
    );
    let id = env.requests.last().unwrap().0;
    hub.tick(
        1_700_000_002,
        vec![PluginEvent::HttpDone {
            id,
            result: response(400, json!({"ok":false})),
        }],
        &mut env,
    );
    hub.lookup(42, 1_700_000_003);
    assert!(hub.aux.is_empty());
    let mut restarted = Hub::default();
    restarted
        .configure(
            &[("token".into(), "secret".into())],
            1_700_000_004,
            &mut env,
        )
        .unwrap();
    restarted.lookup(42, 1_700_000_004);
    assert!(restarted.aux.is_empty());
    restarted.lookup(42, 1_700_000_004 + 86400);
    assert_eq!(restarted.aux.len(), 1);
}
#[test]
fn reconfigure_cancels_inflight_plugin_tasks_and_ignores_stale_completions() {
    let (mut hub, mut env) = authenticated();
    hub.configure(&[("token".into(), "new".into())], 1_700_000_001, &mut env)
        .unwrap();
    assert_eq!(env.cancelled, vec![2]);
    hub.tick(
        1_700_000_002,
        vec![done(2, json!([update(1, "stale")]))],
        &mut env,
    );
    assert_eq!(hub.offset, 0);
}

#[test]
fn rename_and_offset_changes_survive_reconfigure_inside_the_debounce_window() {
    let (mut hub, mut env) = authenticated();
    env.fail_storage = true;
    hub.tick(
        1_700_000_001,
        vec![done(2, json!([update(20, "x")]))],
        &mut env,
    );
    assert!(!env.files.contains_key("offset.json"));
    env.fail_storage = false;
    hub.configure(&[("token".into(), "new".into())], 1_700_000_002, &mut env)
        .unwrap();
    assert_eq!(hub.offset, 21);
    assert_eq!(hub.directory.chats[&-100].name, "Home");
    assert!(env.files.contains_key("directory.json"));
}

#[test]
fn getchat_queued_before_an_update_is_skipped_once_the_name_is_known() {
    let (mut hub, mut env) = authenticated();
    hub.lookup(-100, 1_700_000_001);
    hub.tick(
        1_700_000_001,
        vec![done(2, json!([update(1, "hi")]))],
        &mut env,
    );
    assert!(
        !env.requests
            .iter()
            .any(|(_, request)| request.url.ends_with("getChat"))
    );
}
