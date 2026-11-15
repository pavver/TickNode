//! Abortable HTTP transport shared by node-owned and plugin-owned tasks.
use super::*;
use std::sync::Arc;

const MAX_HTTP_BODY: usize = 4 * 1024 * 1024;

pub(crate) fn spawn_http(
    runtime: &tokio::runtime::Handle,
    request: HttpRequest,
    blob_store: Arc<dyn crate::blobs::BlobStore>,
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
    let mut req = client.request(method, &request.url).timeout(timeout);

    if let Ok(parsed_url) = url::Url::parse(&request.url) {
        let user = parsed_url.username();
        if !user.is_empty() {
            let has_auth = request
                .headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("authorization"));
            if !has_auth {
                req = req.basic_auth(user, parsed_url.password());
            }
        }
    }

    if !request.multipart.is_empty() {
        let mut form = reqwest::multipart::Form::new();
        for part in request.multipart {
            let mut p = match part.data {
                PartData::Text(t) => reqwest::multipart::Part::text(t),
                PartData::Blob(id) => {
                    let bytes = blob_store
                        .read_all(&id)
                        .map_err(|_| "unknown blob".to_string())?;
                    reqwest::multipart::Part::bytes(bytes)
                }
            };
            if let Some(file_name) = part.file_name {
                p = p.file_name(file_name);
            }
            if let Some(content_type) = part.content_type {
                p = p.mime_str(&content_type).map_err(|e| e.to_string())?;
            }
            form = form.part(part.name, p);
        }
        req = req.multipart(form);
        for (name, value) in request.headers {
            if !name.eq_ignore_ascii_case("content-type") {
                req = req.header(name, value);
            }
        }
    } else {
        req = req.body(request.body);
        for (name, value) in request.headers {
            req = req.header(name, value);
        }
    }

    let save_to_blob = request.save_to_blob;
    let req_url = request.url.clone();

    Ok(runtime.spawn(async move {
        let mut response = req.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter_map(|(n, v)| Some((n.to_string(), v.to_str().ok()?.to_string())))
            .collect();

        if save_to_blob && (200..300).contains(&status) {
            let mut body = Vec::new();
            while (body.len() as u64) < crate::blobs::BLOB_STORE_QUOTA {
                let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? else {
                    break;
                };
                if (body.len() as u64).saturating_add(chunk.len() as u64)
                    > crate::blobs::BLOB_STORE_QUOTA
                {
                    return Err("Response body exceeds maximum blob quota".into());
                }
                body.extend_from_slice(&chunk);
            }

            let content_type = headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
                .map(|(_, v)| v.clone());
            let file_name = parse_filename(&headers, &req_url);

            let info = blob_store
                .put(body, content_type.clone(), file_name)
                .map_err(|e| e.to_string())?;

            Ok(TaskOutput::Blob(BlobResponse {
                status,
                headers,
                blob: info.id,
                size: info.size,
                content_type,
            }))
        } else {
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
        }
    }))
}

pub(crate) fn parse_filename(headers: &[(String, String)], url: &str) -> Option<String> {
    if let Some((_, disp)) = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-disposition"))
    {
        for part in disp.split(';') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("filename*=") {
                let rest = rest.trim_matches('"');
                if let Some((_, val)) = rest.split_once("''") {
                    return Some(val.to_string());
                }
                return Some(rest.to_string());
            } else if let Some(rest) = part.strip_prefix("filename=") {
                let rest = rest.trim().trim_matches('"');
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
        }
    }
    if let Ok(parsed) = url::Url::parse(url)
        && let Some(seg) = parsed
            .path_segments()
            .and_then(|mut s| s.rfind(|seg| !seg.is_empty()))
    {
        return Some(seg.to_string());
    }
    None
}
