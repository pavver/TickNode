//! Abortable HTTP transport shared by node-owned and plugin-owned tasks.
use super::*;
use std::sync::Arc;

mod body;
mod request;
use futures::TryStreamExt;

const MAX_HTTP_BODY: usize = 4 * 1024 * 1024;

pub(crate) fn spawn_http(
    runtime: &tokio::runtime::Handle,
    request: HttpRequest,
    blob_store: Arc<dyn crate::blobs::BlobStore>,
) -> Result<tokio::task::JoinHandle<TaskResult>, String> {
    let save_to_blob = request.save_to_blob;
    let req_url = request.url.clone();
    let req = request::build_request(request, blob_store.as_ref())?;

    Ok(runtime.spawn(async move {
        let response = req.send().await.map_err(request_error)?;
        receive_response(response, save_to_blob, &req_url, blob_store).await
    }))
}

async fn receive_response(
    response: reqwest::Response,
    save_to_blob: bool,
    req_url: &str,
    blob_store: Arc<dyn crate::blobs::BlobStore>,
) -> TaskResult {
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .filter_map(|(n, v)| Some((n.to_string(), v.to_str().ok()?.to_string())))
        .collect();

    if save_to_blob && (200..300).contains(&status) {
        let body = body::read_blob_body(
            response.bytes_stream().map_err(request_error),
            crate::blobs::BLOB_STORE_QUOTA as usize,
        )
        .await?;

        let content_type = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());
        let file_name = parse_filename(&headers, req_url);

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
        let body = body::read_http_body(
            response.bytes_stream().map_err(request_error),
            MAX_HTTP_BODY,
        )
        .await?;
        Ok(TaskOutput::Http(HttpResponse {
            status,
            headers,
            body,
        }))
    }
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

/// reqwest errors embed the request URL, which may carry secrets (a bot token in the path,
/// `user:password@` credentials); task errors reach node outputs and logs, so drop it.
fn request_error(error: reqwest::Error) -> String {
    error.without_url().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::BlobStore;

    #[test]
    fn filenames_use_disposition_then_url_path_without_credentials_or_query() {
        for (headers, url, expected) in [
            (
                vec![(
                    "Content-Disposition".into(),
                    "attachment; filename=\"image.jpg\"".into(),
                )],
                "https://example.com/fallback",
                Some("image.jpg"),
            ),
            (
                vec![(
                    "content-disposition".into(),
                    "attachment; filename*=UTF-8''photo.png".into(),
                )],
                "https://example.com/",
                Some("photo.png"),
            ),
            (
                vec![("content-disposition".into(), "filename*=raw.txt".into())],
                "https://example.com/",
                Some("raw.txt"),
            ),
            (
                vec![("content-disposition".into(), "filename=\"\"".into())],
                "https://user:password@example.com/path/file.txt?token=secret",
                Some("file.txt"),
            ),
            (
                vec![],
                "https://example.com/path/file.txt/",
                Some("file.txt"),
            ),
            (vec![], "https://example.com/", None),
            (vec![], "not a URL", None),
        ] {
            assert_eq!(parse_filename(&headers, url).as_deref(), expected);
        }
    }

    #[test]
    fn request_building_applies_auth_headers_timeouts_and_blob_validation_without_sockets() {
        let store = crate::blobs::MemoryBlobStore::new();
        let mut input = HttpRequest::get("http://user:pass@example.com/image");
        input.timeout = chrono::TimeDelta::zero().to_std().unwrap();
        let request = request::build_request(input.clone(), &store)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.headers()["authorization"], "Basic dXNlcjpwYXNz");
        assert_eq!(request.timeout(), Some(&DEFAULT_HTTP_TIMEOUT));
        input.timeout = chrono::TimeDelta::seconds(1000).to_std().unwrap();
        input.headers = vec![("aUtHoRiZaTiOn".into(), "Bearer explicit".into())];
        let request = request::build_request(input.clone(), &store)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.headers()["authorization"], "Bearer explicit");
        assert_eq!(request.timeout(), Some(&MAX_HTTP_TIMEOUT));
        input.multipart.push(MultipartPart {
            name: "file".into(),
            file_name: None,
            content_type: None,
            data: PartData::Blob("missing".into()),
        });
        assert_eq!(
            request::build_request(input.clone(), &store).err().unwrap(),
            "unknown blob"
        );
        let mut ordinary = HttpRequest::get("https://example.com/");
        ordinary
            .headers
            .push(("Content-Type".into(), "application/json".into()));
        let ordinary = request::build_request(ordinary, &store)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(ordinary.headers()["content-type"], "application/json");
        assert!(!ordinary.headers().contains_key("authorization"));
        let blob = store.put(vec![1, 2], None, None).unwrap();
        input.multipart[0].data = PartData::Blob(blob.id);
        input.multipart[0].file_name = Some("image.png".into());
        input.multipart[0].content_type = Some("image/png".into());
        input.multipart.push(MultipartPart {
            name: "caption".into(),
            file_name: None,
            content_type: None,
            data: PartData::Text("hello".into()),
        });
        input
            .headers
            .push(("Content-Type".into(), "application/json".into()));
        let request = request::build_request(input, &store)
            .unwrap()
            .build()
            .unwrap();
        assert!(
            request.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("multipart/form-data; boundary=")
        );
        assert_eq!(request.headers()["authorization"], "Bearer explicit");
    }

    /// Needs loopback networking: connects to a closed local port.
    #[test]
    fn transport_errors_never_contain_the_url_or_its_secrets() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let request = HttpRequest::get("http://user:s3cret@127.0.0.1:1/botTOKEN123/getMe");
        let store: Arc<dyn crate::blobs::BlobStore> =
            Arc::new(crate::blobs::MemoryBlobStore::new());
        let task = spawn_http(runtime.handle(), request, store).expect("spawned");
        let error = runtime
            .block_on(task)
            .expect("joined")
            .expect_err("closed port");
        assert!(!error.contains("TOKEN123"), "{error}");
        assert!(!error.contains("s3cret"), "{error}");
    }
}

#[cfg(test)]
mod response_tests;
