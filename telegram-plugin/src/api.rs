//! JSON Bot API requests and sanitized failure classification.
use serde_json::{Value, json};
use ticknode_plugin_sdk::*;

pub const BASE: &str = "https://api.telegram.org";

pub fn request(base: &str, token: &str, method: &str, body: Value) -> HttpRequest {
    let mut request = HttpRequest::post(format!("{base}/bot{token}/{method}"), body.to_string())
        .header("Content-Type", "application/json");
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
