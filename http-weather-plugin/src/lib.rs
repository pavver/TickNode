//! Example network plugin: fetches city temperature over HTTP.
//!
//! The service at the `url` property (`{city}` is replaced with the city name) is expected to respond
//! with plain text `temperature,description`, for example `22.5,Sunny`.

use ticknode_plugin_sdk::*;

const DEFAULT_URL: &str = "http://localhost/weather?city={city}";

struct WeatherPlugin;

struct WeatherNode {
    url: String,
    pending: Option<TaskId>,
}

const TRIGGER: PinId = PinId(0);
const CITY: PinId = PinId(1);
const SUCCESS: PinId = PinId(2);
const FAILURE: PinId = PinId(3);
const TEMP: PinId = PinId(4);
const DESCRIPTION: PinId = PinId(5);

fn parse(body: &str) -> Option<(f64, String)> {
    let (temp, description) = body.trim().split_once(',')?;
    Some((temp.trim().parse().ok()?, description.trim().to_string()))
}

impl PluginNode for WeatherNode {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 100,
            properties: Vec::new(),
            type_name: "http/weather".into(),
            title: "Get Weather".into(),
            category: "Network".into(),
            description: "Fetches the temperature for a city".into(),
            help: "On a signal at Trigger, makes an HTTP request to the address from the url property: {city} is replaced with City (without City — Kyiv). The service must respond with the text \"temperature,description\", for example `22.5,Sunny`.\n\n• Success — Temp, Description and a signal on Success.\n• A network error, a 4xx/5xx status or an unintelligible response — a signal on Failure.\n\nA new signal cancels the previous unfinished request. The plugin can only access the domains allowed in its manifest.".into(),
            pins: vec![
                exec_in(TRIGGER.0, "Trigger"),
                data_in(CITY.0, "City", DataType::String),
                exec_out(SUCCESS.0, "Success"),
                exec_out(FAILURE.0, "Failure"),
                data_out(TEMP.0, "Temp", DataType::Number),
                data_out(DESCRIPTION.0, "Description", DataType::String),
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("url".into(), DataValue::String(self.url.clone()))]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let ("url", DataValue::String(url)) = (name, value) {
            self.url = url;
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        match trigger {
            Trigger::Pin(TRIGGER) => self.request(inputs, &mut HostClient),
            Trigger::Task { id, result } if self.pending == Some(id) => {
                self.complete(result, &mut HostClient)
            }
            _ => Vec::new(),
        }
    }
}

impl Plugin for WeatherPlugin {
    const NODE_TYPES: &'static [&'static str] = &["http/weather"];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] =
        &[("uk", include_str!("../locales/uk.ftl"))];

    fn manifest() -> PluginManifest {
        PluginManifest {
            name: "weather_plugin".into(),
            version: "0.2.0".into(),
            author: "TickNode".into(),
            requested_capabilities: vec![Capability::Http {
                allowed_domains: vec!["api.openweathermap.org".into(), "localhost".into()],
            }],
            settings: vec![],
        }
    }

    fn create(type_name: &str, _node: NodeId) -> Option<Box<dyn PluginNode>> {
        (type_name == "http/weather").then(|| {
            Box::new(WeatherNode {
                url: DEFAULT_URL.into(),
                pending: None,
            }) as Box<dyn PluginNode>
        })
    }
}

export_plugin!(WeatherPlugin);

/// Percent-encoding for a URL component: `Kyiv&units=imperial` stays a literal value rather than injecting query parameters.
fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests;

impl WeatherNode {
    fn request(&mut self, inputs: &Inputs, client: &mut impl Client) -> Vec<Output> {
        let city = match inputs.string(CITY) {
            c if c.is_empty() => "Kyiv".to_string(),
            c => c,
        };
        // A new request supersedes any in-flight one.
        if let Some(task) = self.pending.take() {
            client.cancel(task);
        }
        match client.http(HttpRequest::get(
            self.url.replace("{city}", &encode_component(&city)),
        )) {
            Ok(task) => {
                self.pending = Some(task);
                Vec::new()
            }
            Err(e) => {
                client.error(&e);
                vec![Output::Exec(FAILURE)]
            }
        }
    }
    fn complete(
        &mut self,
        result: Result<TaskOutput, String>,
        client: &mut impl Client,
    ) -> Vec<Output> {
        self.pending = None;
        let parsed = match result {
            Ok(TaskOutput::Http(response)) if response.is_success() => parse(&response.text()),
            Ok(TaskOutput::Http(response)) => {
                client.error(&format!("HTTP {}", response.status));
                None
            }
            Ok(_) => None,
            Err(e) => {
                client.error(&e);
                None
            }
        };
        match parsed {
            Some((temp, description)) => vec![
                Output::Data(
                    TEMP,
                    DataValue::Number(NumberValue::Float(temp).normalize()),
                ),
                Output::Data(DESCRIPTION, DataValue::String(description)),
                Output::Exec(SUCCESS),
            ],
            None => vec![Output::Exec(FAILURE)],
        }
    }
}

trait Client {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String>;
    fn cancel(&mut self, task: TaskId);
    fn error(&mut self, error: &str);
}
struct HostClient;
impl Client for HostClient {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String> {
        host::http(request)
    }
    fn cancel(&mut self, task: TaskId) {
        host::cancel(task);
    }
    fn error(&mut self, error: &str) {
        host::error(error);
    }
}
