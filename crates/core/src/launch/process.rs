//! Running games: one per instance, with their output.

use std::collections::{HashMap, VecDeque};
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
#[ts(export)]
pub struct GameExit {
    pub code: Option<i32>,
    pub tail: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activity {
    Installing,
    Running,
}

/// What each instance is busy with: installing, or running a game (which
/// includes the launch's own install check). One activity per instance, so
/// two writers never touch the same files.
#[derive(Debug, Clone, Default)]
pub struct RunningRegistry(Arc<Mutex<HashMap<String, Activity>>>);

/// Releases the instance when dropped.
#[derive(Debug)]
pub struct RunningGuard {
    reg: RunningRegistry,
    id: String,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.reg
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

impl RunningRegistry {
    fn acquire(&self, id: &str, what: Activity) -> Result<RunningGuard> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(busy) = map.get(id) {
            let err = match busy {
                Activity::Installing => AppError::new("error.instance.installing"),
                Activity::Running => AppError::new("error.launch.already_running"),
            };
            return Err(err.with_param("id", id));
        }
        map.insert(id.to_string(), what);
        Ok(RunningGuard {
            reg: self.clone(),
            id: id.to_string(),
        })
    }

    /// Claim the instance for a launch.
    pub fn try_register(&self, id: &str) -> Result<RunningGuard> {
        self.acquire(id, Activity::Running)
    }

    /// Claim the instance for an install (no game runs meanwhile).
    pub fn try_register_install(&self, id: &str) -> Result<RunningGuard> {
        self.acquire(id, Activity::Installing)
    }

    /// Is a game (or its launch) in progress?
    pub fn is_running(&self, id: &str) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(id) == Some(&Activity::Running)
    }

    /// Instances with a game in progress, sorted.
    pub fn list(&self) -> Vec<String> {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<String> = map
            .iter()
            .filter(|(_, a)| **a == Activity::Running)
            .map(|(k, _)| k.clone())
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_launch_exclude_each_other() {
        let reg = RunningRegistry::default();
        let install = reg.try_register_install("a").unwrap();
        assert_eq!(
            reg.try_register("a").unwrap_err().key,
            "error.instance.installing"
        );
        assert_eq!(
            reg.try_register_install("a").unwrap_err().key,
            "error.instance.installing"
        );
        assert!(!reg.is_running("a"), "installing is not running a game");
        assert!(
            reg.try_register("b").is_ok(),
            "other instances are unaffected"
        );
        drop(install);
        let game = reg.try_register("a").unwrap();
        assert!(reg.is_running("a"));
        assert_eq!(
            reg.try_register_install("a").unwrap_err().key,
            "error.launch.already_running"
        );
        assert_eq!(reg.list(), vec!["a".to_string()]);
        drop(game);
        assert!(reg.list().is_empty());
    }
}
