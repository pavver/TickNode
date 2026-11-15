//! HTTP tasks are owned by an instance and cancelled when that instance is dropped.
use super::wit;
use crate::host::{HttpRequest, TaskId, TaskOutput, TaskResult, spawn_http};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) const MAX_PLUGIN_HTTP_TASKS: usize = 8;
// Disjoint from node handles and never reused by a recreated instance.
static NEXT_TASK: AtomicU64 = AtomicU64::new(1 << 63);

#[derive(Default)]
pub(super) struct Tasks {
    pub(super) runtime: Option<tokio::runtime::Handle>,
    pub(super) blob_store: Option<Arc<dyn crate::blobs::BlobStore>>,
    pending: BTreeMap<TaskId, tokio::task::JoinHandle<TaskResult>>,
}
impl Tasks {
    pub(super) fn new(
        runtime: Option<tokio::runtime::Handle>,
        blob_store: Option<Arc<dyn crate::blobs::BlobStore>>,
    ) -> Self {
        Self {
            runtime,
            blob_store,
            pending: BTreeMap::new(),
        }
    }
    pub(super) fn start(&mut self, request: HttpRequest) -> Result<TaskId, String> {
        if self.pending.len() >= MAX_PLUGIN_HTTP_TASKS {
            return Err(format!(
                "Too many plugin-owned HTTP tasks (maximum {MAX_PLUGIN_HTTP_TASKS})"
            ));
        }
        let runtime = self
            .runtime
            .clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok())
            .ok_or("HTTP unavailable: the engine is running without a tokio runtime")?;
        let blob_store = self
            .blob_store
            .clone()
            .unwrap_or_else(|| Arc::new(crate::blobs::MemoryBlobStore::default()));
        let task = spawn_http(&runtime, request, blob_store)?;
        let id = NEXT_TASK.fetch_add(1, Ordering::Relaxed);
        self.pending.insert(id, task);
        Ok(id)
    }
    pub(super) fn cancel(&mut self, id: TaskId) -> bool {
        self.pending.remove(&id).is_some_and(|task| {
            task.abort();
            true
        })
    }
    pub(super) fn drain(&mut self, max: usize) -> Vec<wit::WsEvent> {
        let ready: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, t)| t.is_finished())
            .take(max)
            .map(|(id, _)| *id)
            .collect();
        ready
            .into_iter()
            .map(|id| {
                let task = self.pending.remove(&id).expect("finished task exists");
                let result = futures::executor::block_on(task)
                    .unwrap_or_else(|e| Err(format!("Task terminated abnormally: {e}")));
                wit::WsEvent::HttpDone(done(id, result))
            })
            .collect()
    }
}
impl Drop for Tasks {
    fn drop(&mut self) {
        for task in self.pending.values() {
            task.abort();
        }
    }
}
pub(super) fn done(id: TaskId, result: TaskResult) -> wit::TaskDone {
    wit::TaskDone {
        id,
        result: result.map(|output| match output {
            TaskOutput::Delay => wit::TaskOutput::Delay,
            TaskOutput::Http(r) => wit::TaskOutput::Http(wit::HttpResponse {
                status: r.status,
                headers: r.headers,
                body: r.body,
            }),
            TaskOutput::Blob(b) => wit::TaskOutput::Blob(wit::BlobResponse {
                status: b.status,
                headers: b.headers,
                blob: b.blob,
                size: b.size,
                content_type: b.content_type,
            }),
        }),
    }
}

#[cfg(test)]
mod tests;
