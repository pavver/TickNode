use super::{BlobError, BlobInfo};
use std::collections::BTreeMap;

/// Shared quota accounting and deterministic oldest-first eviction for both stores.
pub(super) struct BlobIndex<T> {
    pub blobs: BTreeMap<String, (BlobInfo, T)>,
    total_size: u64,
    quota: u64,
}

impl<T> BlobIndex<T> {
    pub fn new(quota: u64) -> Self {
        Self {
            blobs: BTreeMap::new(),
            total_size: 0,
            quota,
        }
    }

    pub fn insert(&mut self, info: BlobInfo, data: T) {
        self.total_size = self.total_size.saturating_add(info.size);
        self.blobs.insert(info.id.clone(), (info, data));
    }

    pub fn make_room(&mut self, size: u64) -> Result<Vec<BlobInfo>, BlobError> {
        if size > self.quota {
            return Err(BlobError::TooLarge(size, self.quota));
        }
        let mut evicted = Vec::new();
        while self.total_size.saturating_add(size) > self.quota {
            let Some(info) = self.pop_oldest() else {
                break;
            };
            evicted.push(info);
        }
        Ok(evicted)
    }

    fn pop_oldest(&mut self) -> Option<BlobInfo> {
        let id = self
            .blobs
            .values()
            .min_by_key(|(info, _)| (info.created, &info.id))?
            .0
            .id
            .clone();
        let (info, _) = self.blobs.remove(&id)?;
        self.total_size = self.total_size.saturating_sub(info.size);
        tracing::info!(id = %info.id, size = info.size, "evicted blob");
        Some(info)
    }
}
