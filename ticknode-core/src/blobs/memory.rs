use super::index::BlobIndex;
use super::{BLOB_STORE_QUOTA, BlobError, BlobInfo, BlobStore};
use std::sync::Mutex;

/// In-memory blob store for tests and data-dir-less engines.
pub struct MemoryBlobStore {
    inner: Mutex<BlobIndex<Vec<u8>>>,
}

impl MemoryBlobStore {
    pub fn new() -> Self {
        Self::with_quota(BLOB_STORE_QUOTA)
    }

    pub fn with_quota(quota: u64) -> Self {
        Self {
            inner: Mutex::new(BlobIndex::new(quota)),
        }
    }
}

impl Default for MemoryBlobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BlobStore for MemoryBlobStore {
    fn put(
        &self,
        bytes: Vec<u8>,
        content_type: Option<String>,
        file_name: Option<String>,
    ) -> Result<BlobInfo, BlobError> {
        let size = bytes.len() as u64;
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        inner.make_room(size)?;
        let info = BlobInfo::new(size, content_type, file_name);
        inner.insert(info.clone(), bytes);

        Ok(info)
    }

    fn read(&self, id: &str, offset: u64, len: u32) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let (_, data) = inner
            .blobs
            .get(id)
            .ok_or_else(|| BlobError::NotFound(id.to_string()))?;

        if offset >= data.len() as u64 {
            return Ok(Vec::new());
        }

        let start = offset as usize;
        let end = (start.saturating_add(len as usize)).min(data.len());
        Ok(data[start..end].to_vec())
    }

    fn info(&self, id: &str) -> Option<BlobInfo> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.blobs.get(id).map(|(info, _)| info.clone())
    }

    fn read_all(&self, id: &str) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner
            .blobs
            .get(id)
            .map(|(_, data)| data.clone())
            .ok_or_else(|| BlobError::NotFound(id.to_string()))
    }
}
