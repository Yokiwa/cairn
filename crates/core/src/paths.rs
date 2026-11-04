use std::path::{Path, PathBuf};

use crate::{AppError, Result};

/// Environment variable that overrides the data directory (tests, CLI).
pub const HOME_ENV: &str = "CAIRN_HOME";

/// Where everything lives on disk (spec section 3, "数据存放").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

pub(crate) fn default_root_for_os() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().ok_or_else(|| {
        AppError::new("error.io").with_detail("cannot find the user data directory")
    })?;
    let name = if cfg!(target_os = "linux") {
        "cairn"
    } else {
        "Cairn"
    };
    Ok(base.data_dir().join(name))
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `CAIRN_HOME` if set, else the per-OS data directory.
    pub fn default_root() -> Result<PathBuf> {
        match std::env::var_os(HOME_ENV) {
            Some(v) if !v.is_empty() => Ok(PathBuf::from(v)),
            _ => default_root_for_os(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(id)
    }
    pub fn trash_dir(&self) -> PathBuf {
        self.root.join("trash")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("shared").join("versions")
    }
    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("shared").join("libraries")
    }
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("shared").join("assets")
    }
    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_spec() {
        let p = Paths::new(PathBuf::from("/r"));
        assert_eq!(p.settings_file(), PathBuf::from("/r/settings.json"));
        assert_eq!(p.accounts_file(), PathBuf::from("/r/accounts.json"));
        assert_eq!(p.instance_dir("x"), PathBuf::from("/r/instances/x"));
        assert_eq!(p.versions_dir(), PathBuf::from("/r/shared/versions"));
        assert_eq!(p.libraries_dir(), PathBuf::from("/r/shared/libraries"));
        assert_eq!(p.assets_dir(), PathBuf::from("/r/shared/assets"));
        assert_eq!(p.java_dir(), PathBuf::from("/r/java"));
        assert_eq!(p.backups_dir(), PathBuf::from("/r/backups"));
        assert_eq!(p.cache_dir(), PathBuf::from("/r/cache"));
        assert_eq!(p.logs_dir(), PathBuf::from("/r/logs"));
        assert_eq!(p.trash_dir(), PathBuf::from("/r/trash"));
    }

    #[test]
    fn cairn_home_overrides_default_root() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: test-only; no other test reads CAIRN_HOME concurrently.
        unsafe { std::env::set_var(HOME_ENV, dir.path()) };
        let root = Paths::default_root().unwrap();
        unsafe { std::env::remove_var(HOME_ENV) };
        assert_eq!(root, dir.path());
    }

    #[test]
    fn default_root_name_per_os() {
        let root = default_root_for_os().unwrap();
        let name = root.file_name().unwrap().to_string_lossy().to_string();
        if cfg!(target_os = "linux") {
            assert_eq!(name, "cairn");
        } else {
            assert_eq!(name, "Cairn");
        }
    }
}
