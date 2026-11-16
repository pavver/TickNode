use super::*;
use std::fs;

#[test]
fn memory_blob_store_put_read_info() {
    let store = MemoryBlobStore::new();
    let data = b"hello blob world".to_vec();
    let info = store
        .put(
            data.clone(),
            Some("text/plain".into()),
            Some("hello.txt".into()),
        )
        .unwrap();

    assert_eq!(info.size, 16);
    assert_eq!(info.content_type.as_deref(), Some("text/plain"));
    assert_eq!(info.file_name.as_deref(), Some("hello.txt"));

    let read_info = store.info(&info.id).unwrap();
    assert_eq!(read_info, info);

    let all = store.read_all(&info.id).unwrap();
    assert_eq!(all, data);

    let range = store.read(&info.id, 6, 4).unwrap();
    assert_eq!(range, b"blob");

    let past_eof = store.read(&info.id, 100, 10).unwrap();
    assert!(past_eof.is_empty());
}

#[test]
fn memory_blob_store_eviction_and_too_large() {
    let store = MemoryBlobStore::with_quota(20);

    // Too large
    let err = store.put(vec![0u8; 25], None, None).unwrap_err();
    assert!(matches!(err, BlobError::TooLarge(25, 20)));

    // Store first 12 bytes
    let info1 = store.put(vec![1u8; 12], None, None).unwrap();
    assert!(store.info(&info1.id).is_some());

    // Small delay to ensure timestamp/ordering
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Store second 12 bytes -> evicts first
    let info2 = store.put(vec![2u8; 12], None, None).unwrap();
    assert!(store.info(&info1.id).is_none());
    assert!(store.info(&info2.id).is_some());
}

#[test]
fn file_blob_store_put_read_info() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = FileBlobStore::open(temp_dir.path()).unwrap();

    let data = b"file blob content".to_vec();
    let info = store
        .put(
            data.clone(),
            Some("application/octet-stream".into()),
            Some("file.bin".into()),
        )
        .unwrap();

    assert_eq!(info.size, data.len() as u64);
    assert_eq!(store.info(&info.id).unwrap(), info);
    assert_eq!(store.read_all(&info.id).unwrap(), data);

    let part = store.read(&info.id, 5, 4).unwrap();
    assert_eq!(part, b"blob");
}

#[test]
fn file_blob_store_eviction_and_too_large() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = FileBlobStore::open_with_quota(temp_dir.path(), 30).unwrap();

    // Reject larger than quota
    let err = store.put(vec![0u8; 35], None, None).unwrap_err();
    assert!(matches!(err, BlobError::TooLarge(35, 30)));

    let b1 = store.put(vec![1u8; 15], None, None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let b2 = store.put(vec![2u8; 10], None, None).unwrap();
    assert!(store.info(&b1.id).is_some());
    assert!(store.info(&b2.id).is_some());

    std::thread::sleep(std::time::Duration::from_millis(10));
    // Now inserting 15 bytes exceeds 30 (15 + 10 + 15 = 40), so oldest (b1) should be evicted
    let b3 = store.put(vec![3u8; 15], None, None).unwrap();
    assert!(store.info(&b1.id).is_none());
    assert!(store.info(&b2.id).is_some());
    assert!(store.info(&b3.id).is_some());

    // Verify files on disk: b1 file and sidecar deleted
    assert!(!temp_dir.path().join(&b1.id).exists());
    assert!(!temp_dir.path().join(format!("{}.json", b1.id)).exists());
    assert!(temp_dir.path().join(&b2.id).exists());
    assert!(temp_dir.path().join(&b3.id).exists());
}

#[test]
fn file_blob_store_restart_scan_and_orphan_cleanup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir = temp_dir.path();

    {
        let store = FileBlobStore::open(dir).unwrap();
        let _ = store.put(b"valid data".to_vec(), None, None).unwrap();
    }

    // Add orphan tmp file
    fs::write(dir.join("orphan.tmp"), b"junk").unwrap();
    fs::write(dir.join("orphan.json.tmp"), b"junk").unwrap();

    // Add orphan file without sidecar
    fs::write(dir.join("no_sidecar"), b"lonely").unwrap();

    // Add corrupted sidecar
    fs::write(dir.join("corrupt.json"), b"not valid json").unwrap();
    fs::write(dir.join("corrupt"), b"something").unwrap();

    // Reopen store
    let _store2 = FileBlobStore::open(dir).unwrap();

    // Check orphan tmp and invalid files cleaned up
    assert!(!dir.join("orphan.tmp").exists());
    assert!(!dir.join("orphan.json.tmp").exists());
    assert!(!dir.join("no_sidecar").exists());
    assert!(!dir.join("corrupt.json").exists());
    assert!(!dir.join("corrupt").exists());

    // The valid blob still exists
    let entries: Vec<_> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).collect();
    // One data file and one sidecar
    assert_eq!(entries.len(), 2);
}

#[test]
fn memory_and_file_store_parity() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_store = FileBlobStore::open_with_quota(temp_dir.path(), 50).unwrap();
    let mem_store = MemoryBlobStore::with_quota(50);

    let data = b"parity check 123456789".to_vec();
    let file_info = file_store
        .put(data.clone(), Some("text/plain".into()), None)
        .unwrap();
    let mem_info = mem_store
        .put(data.clone(), Some("text/plain".into()), None)
        .unwrap();

    assert_eq!(file_info.size, mem_info.size);
    assert_eq!(file_info.content_type, mem_info.content_type);

    assert_eq!(
        file_store.read(&file_info.id, 0, 6).unwrap(),
        mem_store.read(&mem_info.id, 0, 6).unwrap()
    );
    assert_eq!(
        file_store.read(&file_info.id, 7, 5).unwrap(),
        mem_store.read(&mem_info.id, 7, 5).unwrap()
    );
    assert_eq!(
        file_store.read_all(&file_info.id).unwrap(),
        mem_store.read_all(&mem_info.id).unwrap()
    );
}

#[test]
fn restart_rejects_sidecar_id_that_does_not_match_its_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = FileBlobStore::open(temp.path()).unwrap();
    let info = store.put(b"payload".to_vec(), None, None).unwrap();
    drop(store);
    let mut altered = info.clone();
    altered.id = "../outside".into();
    fs::write(
        temp.path().join(format!("{}.json", info.id)),
        serde_json::to_vec(&altered).unwrap(),
    )
    .unwrap();
    let reopened = FileBlobStore::open(temp.path()).unwrap();
    assert!(reopened.info("../outside").is_none());
    assert!(reopened.info(&info.id).is_none());
    assert!(!temp.path().join(&info.id).exists());
}

#[test]
fn stores_keep_exact_quota_and_reject_oversize_without_eviction() {
    let temp = tempfile::tempdir().unwrap();
    let stores: Vec<Box<dyn BlobStore>> = vec![
        Box::new(MemoryBlobStore::with_quota(4)),
        Box::new(FileBlobStore::open_with_quota(temp.path(), 4).unwrap()),
    ];
    for store in stores {
        let first = store.put(vec![1; 2], None, None).unwrap();
        let second = store.put(vec![2; 2], None, None).unwrap();
        assert_eq!(store.read_all(&first.id).unwrap(), [1; 2]);
        assert_eq!(store.read_all(&second.id).unwrap(), [2; 2]);
        assert!(matches!(
            store.put(vec![3; 5], None, None),
            Err(BlobError::TooLarge(5, 4))
        ));
        assert_eq!(store.read_all(&first.id).unwrap(), [1; 2]);
        assert_eq!(store.read(&second.id, 1, u32::MAX).unwrap(), [2]);
        assert!(store.read(&second.id, 2, 1).unwrap().is_empty());
        assert!(store.read(&second.id, u64::MAX, 1).unwrap().is_empty());
        assert!(store.read(&second.id, 0, 0).unwrap().is_empty());
        assert!(matches!(
            store.read_all("missing"),
            Err(BlobError::NotFound(_))
        ));
        assert!(matches!(
            store.read("missing", 0, 1),
            Err(BlobError::NotFound(_))
        ));
        assert!(store.info("missing").is_none());
    }
}

#[test]
fn restart_enforces_reduced_quota_and_removes_invalid_data() {
    let temp = tempfile::tempdir().unwrap();
    let store = FileBlobStore::open_with_quota(temp.path(), 8).unwrap();
    let first = store.put(vec![1; 4], None, None).unwrap();
    let second = store.put(vec![2; 4], None, None).unwrap();
    drop(store);
    let reopened = FileBlobStore::open_with_quota(temp.path(), 4).unwrap();
    let surviving = [&first, &second]
        .into_iter()
        .filter(|i| reopened.info(&i.id).is_some())
        .collect::<Vec<_>>();
    assert_eq!(surviving.len(), 1);
    let kept = surviving[0];
    assert_eq!(reopened.read_all(&kept.id).unwrap().len(), 4);
    drop(reopened);
    fs::write(temp.path().join(&kept.id), b"short").unwrap();
    let reopened = FileBlobStore::open(temp.path()).unwrap();
    assert!(reopened.info(&kept.id).is_none());
    assert!(!temp.path().join(&kept.id).exists());
}

#[test]
fn zero_quota_accepts_empty_blobs_and_rejects_nonempty_blobs() {
    let temp = tempfile::tempdir().unwrap();
    let stores: Vec<Box<dyn BlobStore>> = vec![
        Box::new(MemoryBlobStore::with_quota(0)),
        Box::new(FileBlobStore::open_with_quota(temp.path(), 0).unwrap()),
    ];
    for store in stores {
        let info = store.put(vec![], None, None).unwrap();
        assert!(store.read_all(&info.id).unwrap().is_empty());
        assert!(matches!(
            store.put(vec![1], None, None),
            Err(BlobError::TooLarge(1, 0))
        ));
        assert!(store.info(&info.id).is_some());
    }
}
