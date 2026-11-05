use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::fsutil::write_atomic;
use crate::{Paths, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Global launcher settings (`settings.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub schema: u32,
    /// `None` follows the system language.
    pub language: Option<String>,
    pub theme: Theme,
    pub accent: String,
    pub download_threads: u8,
    /// `None` uses [`recommended_memory_mb`].
    pub default_memory_mb: Option<u32>,
    pub show_snapshots: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            language: None,
            theme: Theme::System,
            accent: "#34D399".into(),
            download_threads: 32,
            default_memory_mb: None,
            show_snapshots: false,
        }
    }
}

impl Settings {
    fn normalized(mut self) -> Self {
        self.download_threads = self.download_threads.clamp(1, 64);
        self
    }

    /// Missing file → defaults. Unreadable JSON → defaults, and the bad file is
    /// kept as `settings.json.bak` so nothing the person wrote is lost.
    pub fn load(paths: &Paths) -> Result<Settings> {
        let file = paths.settings_file();
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(e.into()),
        };
        match serde_json::from_str::<Settings>(&text) {
            Ok(s) => Ok(s.normalized()),
            Err(_) => {
                std::fs::rename(&file, file.with_file_name("settings.json.bak"))?;
                Ok(Settings::default())
            }
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let s = self.clone().normalized();
        write_atomic(&paths.settings_file(), &serde_json::to_vec_pretty(&s)?)
    }
}

/// Default instance memory from total system memory (spec section 4).
pub fn recommended_memory_mb(total_mb: u64) -> u32 {
    if total_mb <= 8 * 1024 {
        3072
    } else if total_mb <= 16 * 1024 {
        6144
    } else {
        8192
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_paths() -> (tempfile::TempDir, Paths) {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::new(d.path().to_path_buf());
        (d, p)
    }

    #[test]
    fn recommended_memory_tiers() {
        assert_eq!(recommended_memory_mb(8 * 1024), 3072);
        assert_eq!(recommended_memory_mb(4 * 1024), 3072);
        assert_eq!(recommended_memory_mb(16 * 1024), 6144);
        assert_eq!(recommended_memory_mb(32 * 1024), 8192);
    }

    #[test]
    fn settings_default_values() {
        let s = Settings::default();
        assert_eq!(
            (s.download_threads, s.accent.as_str(), s.theme),
            (32, "#34D399", Theme::System)
        );
        assert_eq!(s.language, None);
        assert!(!s.show_snapshots);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_d, p) = tmp_paths();
        assert_eq!(Settings::load(&p).unwrap(), Settings::default());
    }

    #[test]
    fn settings_roundtrip_and_threads_clamped() {
        let (_d, p) = tmp_paths();
        let mut s = Settings {
            language: Some("zh-CN".into()),
            download_threads: 200,
            ..Settings::default()
        };
        s.save(&p).unwrap();
        let back = Settings::load(&p).unwrap();
        assert_eq!(back.language.as_deref(), Some("zh-CN"));
        assert_eq!(back.download_threads, 64);
        s.download_threads = 0;
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p).unwrap().download_threads, 1);
    }

    #[test]
    fn corrupt_settings_falls_back_and_backs_up() {
        let (_d, p) = tmp_paths();
        std::fs::write(p.settings_file(), "{not json").unwrap();
        assert_eq!(Settings::load(&p).unwrap(), Settings::default());
        let bak = p.settings_file().with_file_name("settings.json.bak");
        assert_eq!(std::fs::read_to_string(bak).unwrap(), "{not json");
    }
}
