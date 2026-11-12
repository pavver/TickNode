//! Abortable HTTP transport shared by node-owned and plugin-owned tasks.
use super::*;
const MAX_HTTP_BODY: usize = 4 * 1024 * 1024;

pub(crate) fn spawn_http(
    runtime: &tokio::runtime::Handle,
    request: HttpRequest,
) -> Result<tokio::task::JoinHandle<TaskResult>, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;
    let timeout = if request.timeout.is_zero() {
        DEFAULT_HTTP_TIMEOUT
    } else {
        request.timeout.min(MAX_HTTP_TIMEOUT)
    };
    let method =
        reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut req = client
        .request(method, &request.url)
        .timeout(timeout)
        .body(request.body);
    for (name, value) in request.headers {
        req = req.header(name, value);
    }
    Ok(runtime.spawn(async move {
        let mut response = req.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(n, v)| Some((n.to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let mut body = Vec::new();
        while body.len() < MAX_HTTP_BODY {
            let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? else {
                break;
            };
            body.extend_from_slice(&chunk[..chunk.len().min(MAX_HTTP_BODY - body.len())]);
        }
        Ok(TaskOutput::Http(HttpResponse {
            status,
            headers,
            body,
        }))
    }))
}
