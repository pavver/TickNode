use super::*;
use crate::blobs::{BlobStore, MemoryBlobStore};

fn response(status: u16, body: Vec<u8>) -> reqwest::Response {
    ::http::Response::builder()
        .status(status)
        .header("content-type", "image/png")
        .header("content-disposition", "attachment; filename=photo.png")
        .body(body)
        .unwrap()
        .into()
}

#[tokio::test]
async fn successful_blob_responses_keep_bytes_headers_and_metadata() {
    let store = Arc::new(MemoryBlobStore::new());
    let output = receive_response(
        response(299, vec![1, 2, 3]),
        true,
        "https://example.com/image",
        store.clone(),
    )
    .await
    .unwrap();
    let TaskOutput::Blob(result) = output else {
        panic!("expected blob response")
    };
    assert_eq!(result.status, 299);
    assert_eq!(result.size, 3);
    assert_eq!(result.content_type.as_deref(), Some("image/png"));
    assert!(
        result
            .headers
            .contains(&("content-type".into(), "image/png".into()))
    );
    assert_eq!(store.read_all(&result.blob).unwrap(), [1, 2, 3]);
    assert_eq!(
        store.info(&result.blob).unwrap().file_name.as_deref(),
        Some("photo.png")
    );
}

#[tokio::test]
async fn ordinary_and_unsuccessful_responses_keep_http_bodies_without_blobs() {
    let store = Arc::new(MemoryBlobStore::with_quota(0));
    for (status, save) in [(200, false), (199, true), (300, true), (404, true)] {
        let output = receive_response(response(status, vec![1, 2]), save, "invalid", store.clone())
            .await
            .unwrap();
        let TaskOutput::Http(result) = output else {
            panic!("expected ordinary HTTP response")
        };
        assert_eq!(result.status, status);
        assert_eq!(result.body, [1, 2]);
        assert!(
            result
                .headers
                .contains(&("content-type".into(), "image/png".into()))
        );
    }
}

#[tokio::test]
async fn blob_storage_errors_are_returned_without_partial_success() {
    let result = receive_response(
        response(200, vec![1, 2, 3]),
        true,
        "https://example.com/",
        Arc::new(MemoryBlobStore::with_quota(2)),
    )
    .await;
    assert!(result.is_err());
}
