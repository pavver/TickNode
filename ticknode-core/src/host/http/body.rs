use futures::{Stream, StreamExt};

pub(super) async fn read_blob_body<S, B>(stream: S, limit: usize) -> Result<Vec<u8>, String>
where
    S: Stream<Item = Result<B, String>>,
    B: AsRef<[u8]>,
{
    futures::pin_mut!(stream);
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let bytes = chunk.as_ref();
        if bytes.len() > limit - body.len() {
            return Err("Response body exceeds maximum blob quota".into());
        }
        body.extend_from_slice(bytes);
    }
    Ok(body)
}

pub(super) async fn read_http_body<S, B>(stream: S, limit: usize) -> Result<Vec<u8>, String>
where
    S: Stream<Item = Result<B, String>>,
    B: AsRef<[u8]>,
{
    futures::pin_mut!(stream);
    let mut body = Vec::new();
    while body.len() < limit {
        let Some(chunk) = stream.next().await else {
            break;
        };
        let chunk = chunk?;
        let bytes = chunk.as_ref();
        body.extend_from_slice(&bytes[..bytes.len().min(limit - body.len())]);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor::block_on, stream};

    #[test]
    fn blob_body_rejects_excess_after_exact_limit_chunk() {
        let chunks = stream::iter([Ok(vec![1, 2, 3, 4]), Ok(vec![5])]);
        let result = block_on(read_blob_body(chunks, 4));
        assert!(
            result.is_err(),
            "An extra chunk must never be silently discarded"
        );
    }

    #[test]
    fn blob_body_accepts_exact_limit_and_propagates_stream_errors() {
        let chunks = stream::iter([Ok(vec![1, 2]), Ok(vec![3, 4])]);
        assert_eq!(block_on(read_blob_body(chunks, 4)).unwrap(), [1, 2, 3, 4]);
        let chunks = stream::iter([Ok(vec![1]), Err("connection lost".into())]);
        assert_eq!(
            block_on(read_blob_body(chunks, 4)).unwrap_err(),
            "connection lost"
        );
        assert!(block_on(read_blob_body(stream::iter([Ok(vec![0; 5])]), 4)).is_err());
        assert!(
            block_on(read_blob_body(
                stream::empty::<Result<Vec<u8>, String>>(),
                0
            ))
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn ordinary_http_bodies_keep_the_bounded_prefix() {
        let chunks = stream::iter([Ok(vec![1, 2]), Ok(vec![3, 4, 5])]);
        assert_eq!(block_on(read_http_body(chunks, 4)).unwrap(), [1, 2, 3, 4]);
        let chunks = stream::iter([Ok(vec![1, 2, 3, 4]), Err("must not read after cap".into())]);
        assert_eq!(block_on(read_http_body(chunks, 4)).unwrap(), [1, 2, 3, 4]);
        let chunks = stream::iter([Ok(vec![1])]);
        assert_eq!(block_on(read_http_body(chunks, 4)).unwrap(), [1]);
        let chunks = stream::iter([Err::<Vec<u8>, _>("failed".into())]);
        assert_eq!(block_on(read_http_body(chunks, 4)).unwrap_err(), "failed");
    }
}
