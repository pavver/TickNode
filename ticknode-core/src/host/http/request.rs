use super::*;

pub(super) fn build_request(
    request: HttpRequest,
    store: &dyn crate::blobs::BlobStore,
) -> Result<reqwest::RequestBuilder, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(request_error)?;
    let timeout = if request.timeout.is_zero() {
        DEFAULT_HTTP_TIMEOUT
    } else {
        request.timeout.min(MAX_HTTP_TIMEOUT)
    };
    let method =
        reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
    let builder = client
        .request(method, without_credentials(&request.url))
        .timeout(timeout);
    let builder = basic_auth(builder, &request);
    with_body(builder, request, store)
}

fn without_credentials(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.into();
    };
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.into()
}

fn basic_auth(builder: reqwest::RequestBuilder, request: &HttpRequest) -> reqwest::RequestBuilder {
    let Ok(url) = url::Url::parse(&request.url) else {
        return builder;
    };
    let has_auth = request
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("authorization"));
    if !url.username().is_empty() && !has_auth {
        builder.basic_auth(url.username(), url.password())
    } else {
        builder
    }
}

fn with_body(
    mut builder: reqwest::RequestBuilder,
    request: HttpRequest,
    store: &dyn crate::blobs::BlobStore,
) -> Result<reqwest::RequestBuilder, String> {
    let multipart = !request.multipart.is_empty();
    if multipart {
        let mut form = reqwest::multipart::Form::new();
        for part in request.multipart {
            let name = part.name.clone();
            form = form.part(name, multipart_part(part, store)?);
        }
        builder = builder.multipart(form);
    } else {
        builder = builder.body(request.body);
    }
    for (name, value) in request.headers {
        if !multipart || !name.eq_ignore_ascii_case("content-type") {
            builder = builder.header(name, value);
        }
    }
    Ok(builder)
}

fn multipart_part(
    part: MultipartPart,
    store: &dyn crate::blobs::BlobStore,
) -> Result<reqwest::multipart::Part, String> {
    let mut value = match part.data {
        PartData::Text(text) => reqwest::multipart::Part::text(text),
        PartData::Blob(id) => reqwest::multipart::Part::bytes(
            store
                .read_all(&id)
                .map_err(|_| "unknown blob".to_string())?,
        ),
    };
    if let Some(name) = part.file_name {
        value = value.file_name(name);
    }
    if let Some(mime) = part.content_type {
        value = value.mime_str(&mime).map_err(|e| e.to_string())?;
    }
    Ok(value)
}
