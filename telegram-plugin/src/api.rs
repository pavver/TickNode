//! JSON Bot API requests and sanitized failure classification.
use serde_json::{Value, json};
use ticknode_plugin_sdk::*;

pub const BASE: &str = "https://api.telegram.org";

#[derive(Debug, Clone, PartialEq)]
pub enum RequestBody {
    Json(Value),
    Multipart(Vec<MultipartPart>),
}

impl From<Value> for RequestBody {
    fn from(val: Value) -> Self {
        RequestBody::Json(val)
    }
}

impl RequestBody {
    #[cfg(test)]
    pub fn as_multipart(&self) -> Option<&[MultipartPart]> {
        match self {
            RequestBody::Multipart(p) => Some(p),
            RequestBody::Json(_) => None,
        }
    }

    pub fn chat_id(&self) -> Option<i64> {
        match self {
            RequestBody::Json(v) => v["chat_id"].as_i64(),
            RequestBody::Multipart(parts) => {
                parts.iter().find(|p| p.name == "chat_id").and_then(|p| {
                    if let PartData::Text(ref t) = p.data {
                        t.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
            }
        }
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            RequestBody::Json(Value::Object(map)) => map.get(key),
            _ => None,
        }
    }
}

#[cfg(test)]
impl std::ops::Index<&str> for RequestBody {
    type Output = Value;
    fn index(&self, key: &str) -> &Self::Output {
        match self {
            RequestBody::Json(v) => &v[key],
            RequestBody::Multipart(_) => panic!("cannot index multipart RequestBody: {key}"),
        }
    }
}

pub fn request(base: &str, token: &str, method: &str, body: RequestBody) -> HttpRequest {
    let mut request = match body {
        RequestBody::Json(json) => {
            HttpRequest::post(format!("{base}/bot{token}/{method}"), json.to_string())
                .header("Content-Type", "application/json")
        }
        RequestBody::Multipart(parts) => {
            let mut req = HttpRequest::post(format!("{base}/bot{token}/{method}"), Vec::new());
            req.multipart = parts;
            req
        }
    };
    request.timeout_ms = if method == "getUpdates" {
        60_000
    } else {
        15_000
    };
    request
}

#[derive(Debug, Clone)]
pub struct Failure {
    pub code: u16,
    pub description: String,
    pub retry_after: Option<u64>,
}
impl Failure {
    pub fn ui(&self) -> UiError {
        if self.code == 401 {
            crate::validation::error(
                "unauthorized",
                "Telegram rejected the bot token; update the plugin settings",
            )
        } else {
            UiError::new(
                "telegram-error-api",
                format!("Telegram API error {}: {}", self.code, self.description),
            )
            .arg("code", self.code.to_string())
            .arg("detail", &self.description)
        }
    }
}

pub fn response(result: Result<TaskOutput, String>) -> Result<Value, Failure> {
    let TaskOutput::Http(response) = result.map_err(|_| Failure {
        code: 0,
        description: "HTTP transport failed".into(),
        retry_after: None,
    })?
    else {
        return Err(Failure {
            code: 0,
            description: "Unexpected task result".into(),
            retry_after: None,
        });
    };
    let value: Value = serde_json::from_slice(&response.body).unwrap_or(json!({}));
    if response.is_success() && value["ok"] == true {
        Ok(value["result"].clone())
    } else {
        Err(Failure {
            code: value["error_code"]
                .as_u64()
                .and_then(|n| n.try_into().ok())
                .unwrap_or(response.status),
            description: value["description"]
                .as_str()
                .unwrap_or("Invalid Telegram API response")
                .into(),
            retry_after: value["parameters"]["retry_after"].as_u64(),
        })
    }
}
