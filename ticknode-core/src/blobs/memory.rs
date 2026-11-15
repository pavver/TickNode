use super::{BLOB_STORE_QUOTA, BlobError, BlobInfo, BlobStore};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::info;

struct StoredBlob {
    info: BlobInfo,
    data: Vec<u8>,
}

struct Inner {
    blobs: HashMap<String, StoredBlob>,
    total_size: u64,
    quota: u64,
}

impl Inner {
    fn new(quota: u64) -> Self {
        Self {
            blobs: HashMap::new(),
            total_size: 0,
            quota,
        }
    }

    fn evict_oldest(&mut self) {
        let oldest = self
            .blobs
            .values()
            .min_by(|a, b| (a.info.created, &a.info.id).cmp(&(b.info.created, &b.info.id)))
            .map(|b| (b.info.id.clone(), b.info.size));

        if let Some((id, size)) = oldest {
            info!(id = %id, size = size, "evicted blob");
            self.blobs.remove(&id);
            self.total_size = self.total_size.saturating_sub(size);
        }
    }
}

/// In-memory blob store for tests and data-dir-less engines.
pub struct MemoryBlobStore {
    inner: Mutex<Inner>,
}

impl MemoryBlobStore {
    pub fn new() -> Self {
        Self::with_quota(BLOB_STORE_QUOTA)
    }

    pub fn with_quota(quota: u64) -> Self {
        Self {
            inner: Mutex::new(Inner::new(quota)),
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

        if size > inner.quota {
            return Err(BlobError::TooLarge(size, inner.quota));
        }

        while inner.total_size.saturating_add(size) > inner.quota {
            inner.evict_oldest();
        }

        let id = uuid::Uuid::new_v4().to_string();
        let created = Utc::now();
        let info = BlobInfo {
            id: id.clone(),
            size,
            content_type,
            file_name,
            created,
        };

        inner.total_size = inner.total_size.saturating_add(size);
        inner.blobs.insert(
            id,
            StoredBlob {
                info: info.clone(),
                data: bytes,
            },
        );

        Ok(info)
    }

    fn read(&self, id: &str, offset: u64, len: u32) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let blob = inner
            .blobs
            .get(id)
            .ok_or_else(|| BlobError::NotFound(id.to_string()))?;

        if offset >= blob.data.len() as u64 {
            return Ok(Vec::new());
        }

        let start = offset as usize;
        let end = (start.saturating_add(len as usize)).min(blob.data.len());
        Ok(blob.data[start..end].to_vec())
    }

    fn info(&self, id: &str) -> Option<BlobInfo> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.blobs.get(id).map(|b| b.info.clone())
    }

    fn read_all(&self, id: &str) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner
            .blobs
            .get(id)
            .map(|b| b.data.clone())
            .ok_or_else(|| BlobError::NotFound(id.to_string()))
    }
}
