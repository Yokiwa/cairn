//! Background tasks with progress and cancellation, shared by the app and CLI.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{AppError, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Progress {
    pub stage: String,
    #[ts(type = "number")]
    pub done_files: u64,
    #[ts(type = "number")]
    pub total_files: u64,
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub bytes_per_sec: u64,
}

pub type ProgressFn = Arc<dyn Fn(&Progress) + Send + Sync>;

pub type TaskId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TaskState {
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TaskInfo {
    pub id: TaskId,
    pub title: String,
    pub state: TaskState,
    pub progress: Option<Progress>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TaskEvent {
    Progress {
        id: TaskId,
        progress: Progress,
    },
    Done {
        id: TaskId,
        #[ts(type = "unknown")]
        result: serde_json::Value,
    },
    Failed {
        id: TaskId,
        error: AppError,
    },
}

struct Entry {
    info: TaskInfo,
    cancel: CancellationToken,
}

#[derive(Clone)]
pub struct TaskManager {
    entries: Arc<Mutex<HashMap<TaskId, Entry>>>,
    tx: broadcast::Sender<TaskEvent>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TaskEvent> {
        self.tx.subscribe()
    }

    /// Run `f` in the background. It receives a progress callback and a
    /// cancellation token; its result becomes a `Done` or `Failed` event.
    pub fn spawn<F, Fut>(&self, title: String, f: F) -> TaskId
    where
        F: FnOnce(ProgressFn, CancellationToken) -> Fut,
        Fut: Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = CancellationToken::new();
        self.entries.lock().unwrap().insert(
            id.clone(),
            Entry {
                info: TaskInfo {
                    id: id.clone(),
                    title,
                    state: TaskState::Running,
                    progress: None,
                },
                cancel: cancel.clone(),
            },
        );

        let progress: ProgressFn = {
            let entries = self.entries.clone();
            let tx = self.tx.clone();
            let id = id.clone();
            Arc::new(move |p: &Progress| {
                if let Some(e) = entries.lock().unwrap().get_mut(&id) {
                    e.info.progress = Some(p.clone());
                }
                let _ = tx.send(TaskEvent::Progress {
                    id: id.clone(),
                    progress: p.clone(),
                });
            })
        };

        let fut = f(progress, cancel.clone());
        let entries = self.entries.clone();
        let tx = self.tx.clone();
        let task_id = id.clone();
        tokio::spawn(async move {
            let result = fut.await;
            let event = {
                let mut map = entries.lock().unwrap();
                let entry = map.get_mut(&task_id);
                match result {
                    Ok(v) => {
                        if let Some(e) = entry {
                            e.info.state = TaskState::Done;
                        }
                        TaskEvent::Done {
                            id: task_id.clone(),
                            result: v,
                        }
                    }
                    Err(err) => {
                        if let Some(e) = entry {
                            e.info.state =
                                if err.key == "error.cancelled" || e.cancel.is_cancelled() {
                                    TaskState::Cancelled
                                } else {
                                    TaskState::Failed
                                };
                        }
                        TaskEvent::Failed {
                            id: task_id.clone(),
                            error: err,
                        }
                    }
                }
            };
            let _ = tx.send(event);
        });
        id
    }

    pub fn cancel(&self, id: &str) {
        if let Some(e) = self.entries.lock().unwrap().get(id) {
            e.cancel.cancel();
        }
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        self.entries
            .lock()
            .unwrap()
            .values()
            .map(|e| e.info.clone())
            .collect()
    }
}

pub fn cancelled() -> AppError {
    AppError::new("error.cancelled")
}
