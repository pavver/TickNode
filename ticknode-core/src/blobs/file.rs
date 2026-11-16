use super::index::BlobIndex;
use super::{BLOB_STORE_QUOTA, BlobError, BlobInfo, BlobStore};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{error, warn};

/// Filesystem-backed blob store.
pub struct FileBlobStore {
    dir: PathBuf,
    inner: Mutex<BlobIndex<()>>,
}

impl FileBlobStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, BlobError> {
        Self::open_with_quota(dir, BLOB_STORE_QUOTA)
    }

    pub fn open_with_quota(dir: impl AsRef<Path>, quota: u64) -> Result<Self, BlobError> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        let mut inner = scan(&dir, quota)?;
        for info in inner.make_room(0)? {
            remove_blob_files(&dir, &info.id);
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

        for info in inner.make_room(size)? {
            remove_blob_files(&self.dir, &info.id);
        }
        let info = BlobInfo::new(size, content_type, file_name);
        self.write_blob(&info, &bytes)?;
        inner.insert(info.clone(), ());
        Ok(info)
    }

    fn read(&self, id: &str, offset: u64, len: u32) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let (info, _) = inner
            .blobs
            .get(id)
            .ok_or_else(|| BlobError::NotFound(id.to_string()))?;
        if offset >= info.size {
            return Ok(Vec::new());
        }
        let mut file = fs::File::open(self.dir.join(id)).map_err(|e| read_error(id, e))?;
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
        inner.blobs.get(id).map(|(info, _)| info.clone())
    }

    fn read_all(&self, id: &str) -> Result<Vec<u8>, BlobError> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if !inner.blobs.contains_key(id) {
            return Err(BlobError::NotFound(id.to_string()));
        }

        fs::read(self.dir.join(id)).map_err(|e| read_error(id, e))
    }
}

fn read_error(id: &str, error: std::io::Error) -> BlobError {
    if error.kind() == std::io::ErrorKind::NotFound {
        BlobError::NotFound(id.to_string())
    } else {
        BlobError::Io(error)
    }
}

fn remove_blob_files(dir: &Path, id: &str) {
    for path in [dir.join(id), dir.join(format!("{id}.json"))] {
        if let Err(error) = fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            error!(id, %error, "failed to delete blob file");
        }
    }
}

fn scan(dir: &Path, quota: u64) -> Result<BlobIndex<()>, BlobError> {
    let files = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    let mut index = BlobIndex::new(quota);
    for name in &files {
        if name.ends_with(".tmp") {
            let _ = fs::remove_file(dir.join(name));
        } else if let Some(id) = name.strip_suffix(".json") {
            if let Some(info) = read_sidecar(dir, id) {
                index.insert(info, ());
            } else {
                remove_blob_files(dir, id);
            }
        }
    }
    remove_orphan_files(dir, &files, &index);
    Ok(index)
}

fn remove_orphan_files(dir: &Path, files: &[String], index: &BlobIndex<()>) {
    for name in files {
        if !name.ends_with(".json") && !name.ends_with(".tmp") && !index.blobs.contains_key(name) {
            let _ = fs::remove_file(dir.join(name));
        }
    }
}

fn read_sidecar(dir: &Path, id: &str) -> Option<BlobInfo> {
    let meta_path = dir.join(format!("{id}.json"));
    let bytes = fs::read(&meta_path).ok()?;
    let info: BlobInfo = serde_json::from_slice(&bytes)
        .inspect_err(
            |error| warn!(file = %meta_path.display(), %error, "invalid blob sidecar metadata"),
        )
        .ok()?;
    let data = fs::symlink_metadata(dir.join(id)).ok()?;
    if info.id != id || !data.is_file() || data.len() != info.size {
        warn!(id, "blob data or identity does not match sidecar; removing");
        return None;
    }
    Some(info)
}

impl FileBlobStore {
    fn write_blob(&self, info: &BlobInfo, bytes: &[u8]) -> Result<(), BlobError> {
        let id = &info.id;
        let data_tmp = self.dir.join(format!("{id}.tmp"));
        let meta_tmp = self.dir.join(format!("{id}.json.tmp"));
        let data_final = self.dir.join(id);
        let meta_final = self.dir.join(format!("{id}.json"));
        let result = (|| -> Result<(), BlobError> {
            fs::write(&data_tmp, bytes)?;
            fs::write(&meta_tmp, serde_json::to_vec_pretty(info)?)?;
            fs::rename(&data_tmp, &data_final)?;
            fs::rename(&meta_tmp, &meta_final)?;
            Ok(())
        })();
        if result.is_err() {
            for path in [&data_tmp, &meta_tmp, &data_final, &meta_final] {
                let _ = fs::remove_file(path);
            }
        }
        result
    }
}
