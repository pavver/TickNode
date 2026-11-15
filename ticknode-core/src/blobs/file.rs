use super::{BLOB_STORE_QUOTA, BlobError, BlobInfo, BlobStore};
use chrono::Utc;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{error, info, warn};

struct Inner {
    blobs: BTreeMap<String, BlobInfo>,
    total_size: u64,
    quota: u64,
}

impl Inner {
    fn evict_oldest(&mut self, dir: &Path) {
        let oldest = self
            .blobs
            .values()
            .min_by(|a, b| (a.created, &a.id).cmp(&(b.created, &b.id)))
            .map(|b| (b.id.clone(), b.size));

        if let Some((id, size)) = oldest {
            info!(id = %id, size = size, "evicted blob");
            self.blobs.remove(&id);
            self.total_size = self.total_size.saturating_sub(size);

            let data_path = dir.join(&id);
            let meta_path = dir.join(format!("{id}.json"));
            if let Err(e) = fs::remove_file(&data_path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                error!(id = %id, error = %e, "failed to delete evicted blob file");
            }
            if let Err(e) = fs::remove_file(&meta_path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                error!(id = %id, error = %e, "failed to delete evicted blob sidecar");
            }
        }
    }
}

/// Filesystem-backed blob store.
pub struct FileBlobStore {
    dir: PathBuf,
    inner: Mutex<Inner>,
}

impl FileBlobStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, BlobError> {
        Self::open_with_quota(dir, BLOB_STORE_QUOTA)
    }

    pub fn open_with_quota(dir: impl AsRef<Path>, quota: u64) -> Result<Self, BlobError> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        let mut blobs = BTreeMap::new();
        let mut total_size = 0u64;

        if let Ok(entries) = fs::read_dir(&dir) {
            let mut files = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push(name.to_string());
                }
            }

            // Remove any leftover temporary files.
            for name in &files {
                if name.ends_with(".tmp") {
                    let _ = fs::remove_file(dir.join(name));
                }
            }

            // Inspect sidecar JSON files and correlate with data files.
            for name in &files {
                if !name.ends_with(".json") || name.ends_with(".json.tmp") {
                    continue;
                }
                let id = match name.strip_suffix(".json") {
                    Some(s) => s,
                    None => continue,
                };
                let meta_path = dir.join(name);
                let data_path = dir.join(id);

                let meta_bytes = match fs::read(&meta_path) {
                    Ok(b) => b,
                    Err(_) => {
                        let _ = fs::remove_file(&meta_path);
                        let _ = fs::remove_file(&data_path);
                        continue;
                    }
                };

                let info: BlobInfo = match serde_json::from_slice(&meta_bytes) {
                    Ok(i) => i,
                    Err(e) => {
                        warn!(file = %meta_path.display(), error = %e, "invalid blob sidecar metadata");
                        let _ = fs::remove_file(&meta_path);
                        let _ = fs::remove_file(&data_path);
                        continue;
                    }
                };

                let Ok(data_meta) = fs::metadata(&data_path) else {
                    let _ = fs::remove_file(&meta_path);
                    continue;
                };

                if data_meta.len() != info.size {
                    warn!(id = %id, "blob data file size mismatch with sidecar; removing");
                    let _ = fs::remove_file(&meta_path);
                    let _ = fs::remove_file(&data_path);
                    continue;
                }

                total_size = total_size.saturating_add(info.size);
                blobs.insert(info.id.clone(), info);
            }

            // Remove any data files lacking a sidecar.
            for name in &files {
                if name.ends_with(".json") || name.ends_with(".tmp") {
                    continue;
                }
                if !blobs.contains_key(name) {
                    let _ = fs::remove_file(dir.join(name));
                }
            }
        }

        let mut inner = Inner {
            blobs,
            total_size,
            quota,
        };

        // Enforce quota during startup.
        while inner.total_size > inner.quota {
            inner.evict_oldest(&dir);
        }

        Ok(Self {
            dir,
            inner: Mutex::new(inner),
        })
    }
}

impl BlobStore for FileBlobStore {
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
            inner.evict_oldest(&self.dir);
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

        let data_tmp = self.dir.join(format!("{id}.tmp"));
        let meta_tmp = self.dir.join(format!("{id}.json.tmp"));
        let data_final = self.dir.join(&id);
        let meta_final = self.dir.join(format!("{id}.json"));

        let write_result = (|| -> Result<(), BlobError> {
            fs::write(&data_tmp, &bytes)?;
            let meta_json = serde_json::to_vec_pretty(&info)?;
            fs::write(&meta_tmp, &meta_json)?;
            fs::rename(&data_tmp, &data_final)?;
            fs::rename(&meta_tmp, &meta_final)?;
            Ok(())
        })();

        if let Err(e) = write_result {
            let _ = fs::remove_file(&data_tmp);
            let _ = fs::remove_file(&meta_tmp);
            let _ = fs::remove_file(&data_final);
            let _ = fs::remove_file(&meta_final);
            return Err(e);
        }

        inner.total_size = inner.total_size.saturating_add(size);
        inner.blobs.insert(id, info.clone());

        Ok(info)
    }

    fn read(&self, id: &str, offset: u64, len: u32) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let info = inner
            .blobs
            .get(id)
            .ok_or_else(|| BlobError::NotFound(id.to_string()))?;

        if offset >= info.size {
            return Ok(Vec::new());
        }

        let file_path = self.dir.join(id);
        let mut file = match fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(BlobError::NotFound(id.to_string()));
            }
            Err(e) => return Err(BlobError::Io(e)),
        };

        file.seek(SeekFrom::Start(offset))?;
        let to_read = (info.size - offset).min(len as u64) as usize;
        let mut buf = vec![0u8; to_read];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn info(&self, id: &str) -> Option<BlobInfo> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.blobs.get(id).cloned()
    }

    fn read_all(&self, id: &str) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if !inner.blobs.contains_key(id) {
            return Err(BlobError::NotFound(id.to_string()));
        }

        let file_path = self.dir.join(id);
        match fs::read(&file_path) {
            Ok(b) => Ok(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(BlobError::NotFound(id.to_string()))
            }
            Err(e) => Err(BlobError::Io(e)),
        }
    }
}
