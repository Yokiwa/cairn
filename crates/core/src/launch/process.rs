//! Running games: one per instance, with their output.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AppError, Result};

pub const TAIL_LINES: usize = 50;

pub type LogFn = Arc<dyn Fn(&str) + Send + Sync>;

/// The last [`TAIL_LINES`] lines of a game's output.
#[derive(Debug, Clone, Default)]
pub struct LogTail(Arc<Mutex<VecDeque<String>>>);

impl LogTail {
    pub fn push(&self, line: &str) {
        let mut q = self.0.lock().unwrap();
        if q.len() == TAIL_LINES {
            q.pop_front();
        }
        q.push_back(line.to_string());
    }
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct GameExit {
    pub code: Option<i32>,
    pub tail: Vec<String>,
}

/// Which instances have a game (or a launch in progress) right now.
#[derive(Debug, Clone, Default)]
pub struct RunningRegistry(Arc<Mutex<HashSet<String>>>);

/// Unregisters the instance when dropped.
#[derive(Debug)]
pub struct RunningGuard {
    reg: RunningRegistry,
    id: String,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.reg.0.lock().unwrap().remove(&self.id);
    }
}

impl RunningRegistry {
    pub fn try_register(&self, id: &str) -> Result<RunningGuard> {
        if !self.0.lock().unwrap().insert(id.to_string()) {
            return Err(AppError::new("error.launch.already_running").with_param("id", id));
        }
        Ok(RunningGuard {
            reg: self.clone(),
            id: id.to_string(),
        })
    }
    pub fn is_running(&self, id: &str) -> bool {
        self.0.lock().unwrap().contains(id)
    }
    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<String> = self.0.lock().unwrap().iter().cloned().collect();
        v.sort();
        v
    }
}
