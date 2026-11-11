//! Scripted in-memory Telegram API and filesystem, shared by host-target tests.
use crate::{
    Telegram,
    hub::{Env, Hub},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use ticknode_plugin_sdk::*;

#[derive(Default)]
pub(crate) struct Fake {
    pub requests: Vec<(TaskId, HttpRequest)>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub writes: Vec<String>,
    pub logs: Vec<String>,
    pub errors: Vec<UiError>,
    pub sources: BTreeMap<String, Vec<PropertyOption>>,
    pub cancelled: Vec<TaskId>,
    pub delays: Vec<(TaskId, u64)>,
    pub fail_storage: bool,
    pub fail_http: bool,
    next: u64,
}
impl Env for Fake {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String> {
        if self.fail_http {
            return Err("Transport unavailable".into());
        }
        self.next += 1;
        self.requests.push((self.next, request));
        Ok(self.next)
    }
    fn delay(&mut self, seconds: u64) -> Result<TaskId, String> {
        self.next += 1;
        self.delays.push((self.next, seconds));
        Ok(self.next)
    }
    fn cancel(&mut self, task: TaskId) {
        self.cancelled.push(task);
    }
    fn log(&mut self, text: &str) {
        self.logs.push(text.into());
    }
    fn settings_error(&mut self, error: &UiError) {
        self.errors.push(error.clone());
    }
    fn options(&mut self, source: &str, options: &[PropertyOption]) {
        self.sources.insert(source.into(), options.into());
    }
    fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.files.get(name).cloned())
    }
    fn write(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        if self.fail_storage {
            return Err("Disk full".into());
        }
        self.files.insert(name.into(), bytes.into());
        self.writes.push(name.into());
        Ok(())
    }
}
pub(crate) fn response(status: u16, value: Value) -> Result<TaskOutput, String> {
    Ok(TaskOutput::Http(HttpResponse {
        status,
        headers: vec![],
        body: value.to_string().into_bytes(),
    }))
}
pub(crate) fn done(id: TaskId, value: Value) -> PluginEvent {
    PluginEvent::HttpDone {
        id,
        result: response(200, json!({"ok":true,"result":value})),
    }
}
pub(crate) fn authenticated() -> (Hub, Fake) {
    let (mut hub, mut env) = (Hub::default(), Fake::default());
    hub.configure(
        &[("token".into(), "secret".into())],
        1_700_000_000,
        &mut env,
    )
    .unwrap();
    hub.tick(
        1_700_000_000,
        vec![done(1, json!({"username":"TickBot"}))],
        &mut env,
    );
    (hub, env)
}
pub(crate) fn message(chat: i64, id: i64, text: &str) -> Value {
    json!({"message_id":id,"date":1700000000,"chat":{"id":chat,"type":"supergroup","title":"Home"},
        "from":{"id":17,"first_name":"Ada","is_bot":false,"username":"ada"},"text":text})
}
pub(crate) fn update(id: i64, text: &str) -> Value {
    json!({"update_id":id,"message":message(-100, 50, text)})
}
pub(crate) fn body(request: &HttpRequest) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}

#[test]
fn settings_validate_required_token_allowlist_and_ttl() {
    let (mut hub, mut env) = (Hub::default(), Fake::default());
    assert_eq!(
        hub.configure(&[], 0, &mut env).unwrap_err().key,
        "telegram-error-token"
    );
    for (key, value) in [
        ("allowed_chats", "x".into()),
        ("directory_ttl_days", (-1i64).into()),
        ("directory_ttl_days", 1.5f64.into()),
    ] {
        assert!(
            hub.configure(
                &[("token".into(), "x".into()), (key.into(), value)],
                0,
                &mut env
            )
            .is_err()
        );
    }
    hub.configure(
        &[
            ("token".into(), "x".into()),
            ("allowed_chats".into(), "-100, 42".into()),
        ],
        0,
        &mut env,
    )
    .unwrap();
    assert!(hub.permits(-100));
    assert!(hub.permits(42));
    assert!(!hub.permits(1));
}
#[test]
fn translations_cover_every_definition_and_error() {
    use fluent_syntax::{ast, parser};
    let sets: Vec<_> = Telegram::TRANSLATIONS
        .iter()
        .map(|(_, text)| {
            let resource = parser::parse(*text).unwrap();
            resource
                .body
                .into_iter()
                .filter_map(|entry| match entry {
                    ast::Entry::Message(m) => Some(m.id.name.to_string()),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>()
        })
        .collect();
    assert_eq!(sets[0], sets[1]);
    let has = |key: String| assert!(sets[0].contains(&key), "Missing {key}");
    for name in Telegram::NODE_TYPES {
        let node = Telegram::create(name, NodeId::generate())
            .unwrap()
            .definition();
        let base = format!("node-{}", slug(name));
        for field in ["title", "description", "help"] {
            has(format!("{base}-{field}"));
        }
        for pin in node.pins {
            let key = if pin
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                pin.name
            } else {
                slug(&pin.name)
            };
            has(format!("{base}-pin-{key}"));
        }
        for prop in node.properties {
            has(format!("{base}-prop-{}-title", prop.name));
            if let PropertyKind::Select(options) = prop.kind {
                for (value, _) in options {
                    has(format!("{base}-prop-{}-option-{}", prop.name, slug(&value)));
                }
            }
        }
    }
    for ty in Telegram::types() {
        let base = format!("type-{}", slug(&ty.name));
        has(format!("{base}-title"));
        has(format!("{base}-description"));
        for field in ty.fields() {
            has(format!("{base}-field-{}", field.name));
        }
    }
    for setting in Telegram::manifest().settings {
        let base = format!("setting-telegram-{}", slug(&setting.key));
        has(format!("{base}-title"));
        has(format!("{base}-description"));
    }
    for suffix in [
        "token",
        "allowed-chats",
        "ttl",
        "storage",
        "unauthorized",
        "connecting",
        "id",
        "command",
        "buttons",
        "regex",
        "toast",
        "choice",
        "property",
        "busy",
        "http",
        "changed",
        "response",
        "chat",
        "message",
        "text",
        "length",
        "forbidden",
        "api",
    ] {
        has(format!("telegram-error-{suffix}"));
    }
}
#[test]
fn property_roundtrip_and_rejection_do_not_mutate_state() {
    for name in Telegram::NODE_TYPES {
        let mut node = Telegram::create(name, NodeId::generate()).unwrap();
        for (key, value) in node.properties() {
            node.set_property(&key, value);
        }
        let props = node.properties();
        let mut restored = Telegram::create(name, NodeId::generate()).unwrap();
        for (key, value) in &props {
            restored.set_property(key, value.clone());
        }
        assert_eq!(restored.properties(), props);
        assert!(node.validate_property("chat", &"not an ID".into()).is_err());
        assert_eq!(node.properties(), props);
    }
}
