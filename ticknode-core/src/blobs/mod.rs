//! Binary media blob storage for host and plugins.
//!
//! Blobs are stored behind a unified total quota (default 200 MB),
//! evicting the oldest blobs first when space is required.

use chrono::{DateTime, Utc};
use std::fmt;

mod file;
mod memory;
#[cfg(test)]
pub(crate) mod tests;

pub use file::FileBlobStore;
pub use memory::MemoryBlobStore;

/// Default total quota: 200 MB.
pub const BLOB_STORE_QUOTA: u64 = 200 * 1024 * 1024;

/// Metadata for a stored blob.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlobInfo {
    pub id: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub created: DateTime<Utc>,
}

/// Errors returned by blob store operations.
#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("Blob not found: {0}")]
    NotFound(String),
    #[error("Blob size {0} bytes exceeds quota of {1} bytes")]
    TooLarge(u64, u64),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Abstract blob storage interface.
pub trait BlobStore: Send + Sync {
    /// Stores binary data, evicting oldest blobs if needed to satisfy the quota.
    fn put(
        &self,
        bytes: Vec<u8>,
        content_type: Option<String>,
        file_name: Option<String>,
    ) -> Result<BlobInfo, BlobError>;

    /// Reads up to `len` bytes starting at `offset`.
    fn read(&self, id: &str, offset: u64, len: u32) -> Result<Vec<u8>, BlobError>;

    /// Retrieves metadata for a blob if it exists.
    fn info(&self, id: &str) -> Option<BlobInfo>;

    /// Reads the entire content of a blob into memory.
    fn read_all(&self, id: &str) -> Result<Vec<u8>, BlobError>;
}

impl fmt::Debug for dyn BlobStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobStore")
    }
}
