//! Plugin-level HTTP probes: configure and tick starts, limits, cancellation and completions.
use std::cell::RefCell;
use ticknode_plugin_sdk::*;

#[derive(Default)]
struct State {
    probes: Vec<NodeId>,
    tasks: Vec<TaskId>,
    status: String,
    body: String,
    completed: i64,
    tick_url: Option<String>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

pub(super) fn configure(settings: &[(String, DataValue)]) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for id in s.tasks.drain(..) {
            host::cancel(id);
        }
        s.completed = 0;
        s.body.clear();
        s.status.clear();
        s.tick_url = setting_bool(settings, "http_tick")
            .unwrap_or(false)
            .then(|| setting_str(settings, "http_url").unwrap_or_default().to_string());
        if s.tick_url.is_some() {
            return;
        }
        let Some(url) = setting_str(settings, "http_url") else { return };
        let count = setting_number(settings, "http_count").map(|n| n as usize).unwrap_or(1).min(10);
        for _ in 0..count {
            match host::http(HttpRequest { timeout_ms: 70_000, ..HttpRequest::get(url) }) {
                Ok(id) => s.tasks.push(id),
                Err(e) => s.status = e,
            }
        }
        let started = s.tasks.len();
        s.status = format!("started:{started};{}", s.status);
        if setting_bool(settings, "http_cancel").unwrap_or(false) {
            let cancelled = s.tasks.drain(..).filter(|id| host::cancel(*id)).count();
            s.status.push_str(&format!(";cancelled:{cancelled}"));
        }
    });
}

pub(super) fn tick(events: &[PluginEvent]) -> Vec<Emission> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(url) = s.tick_url.take() {
            match host::http(HttpRequest::get(url)) {
                Ok(id) => {
                    s.tasks.push(id);
                    s.status = "tick-started".into();
                }
                Err(e) => s.status = e,
            }
        }
        let mut done = false;
        for event in events {
            if let PluginEvent::HttpDone { id, result } = event {
                s.tasks.retain(|task| task != id);
                s.completed += 1;
                s.body = match result {
                    Ok(TaskOutput::Http(r)) => r.text(),
                    other => format!("{other:?}"),
                };
                done = true;
            }
        }
        s.probes
            .iter()
            .map(|node| {
                let mut outputs = vec![
                    Output::Data(PinId(1), s.body.clone().into()),
                    Output::Data(PinId(2), s.status.clone().into()),
                    Output::Data(PinId(3), s.completed.into()),
                ];
                if done {
                    outputs.push(Output::Exec(PinId(4)));
                }
                Emission::new(*node, outputs)
            })
            .collect()
    })
}

pub(super) struct Probe(NodeId);
impl Probe {
    pub(super) fn new(id: NodeId) -> Self {
        if !id.0.is_nil() {
            STATE.with(|s| s.borrow_mut().probes.push(id));
        }
        Self(id)
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().probes.retain(|id| *id != self.0));
    }
}
impl PluginNode for Probe {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/http_probe".into(),
            title: "Plugin HTTP probe".into(),
            category: "Test".into(),
            description: "Observes HTTP owned by the plugin".into(),
            help: "Starts requests in configure or tick and reports completions delivered to plugin-tick.".into(),
            pins: vec![
                data_out(1, "Body", DataType::String),
                data_out(2, "Status", DataType::String),
                data_out(3, "Completed", DataType::Number),
                exec_out(4, "Done"),
            ],
            ..Default::default()
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
}
