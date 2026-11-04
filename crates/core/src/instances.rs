//! Instances: each one a folder with `instance.json` and its own game directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::fsutil::write_atomic;
use crate::{AppError, Paths, Result};

const GAME_SUBDIRS: [&str; 5] = ["mods", "saves", "resourcepacks", "shaderpacks", "config"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct LoaderSpec {
    pub kind: LoaderKind,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(tag = "mode", rename_all = "lowercase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum JavaSetting {
    #[default]
    Auto,
    Path {
        #[ts(type = "string")]
        path: PathBuf,
    },
}

/// Where a mod file was installed from (written by the phase 2 mod tools).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ModSource {
    pub source: String,
    pub project_id: String,
    pub version_id: String,
}

/// `instance.json` (spec section 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Instance {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub minecraft: String,
    pub loader: Option<LoaderSpec>,
    #[serde(default)]
    pub java: JavaSetting,
    pub memory_mb: u32,
    #[serde(default)]
    pub jvm_args: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_played_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mods: BTreeMap<String, ModSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct NewInstance {
    pub name: String,
    pub minecraft: String,
    pub loader: Option<LoaderSpec>,
    pub memory_mb: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ModFile {
    pub file_name: String,
    pub enabled: bool,
    #[ts(type = "number")]
    pub size: u64,
}

/// Lower-case ASCII letters and digits, other runs become one `-`;
/// "instance" when nothing is left (e.g. an all-Chinese name).
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "instance".into()
    } else {
        s
    }
}

#[derive(Debug, Clone)]
pub struct InstanceStore {
    paths: Paths,
}

fn check_file_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(AppError::new("error.instance.bad_file").with_param("file", name));
    }
    Ok(())
}

impl InstanceStore {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    fn json_path(&self, id: &str) -> PathBuf {
        self.paths.instance_dir(id).join("instance.json")
    }

    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.paths.instance_dir(id).join("minecraft")
    }

    /// Every readable instance, plus one error per unreadable one.
    pub fn list(&self) -> Result<(Vec<Instance>, Vec<AppError>)> {
        let mut list = Vec::new();
        let mut errors = Vec::new();
        let rd = match std::fs::read_dir(self.paths.instances_dir()) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((list, errors)),
            Err(e) => return Err(e.into()),
        };
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().to_string();
            match self.get(&id) {
                Ok(i) => list.push(i),
                Err(err) if err.key == "error.instance.not_found" => {}
                Err(err) => errors.push(err),
            }
        }
        list.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        Ok((list, errors))
    }

    pub fn get(&self, id: &str) -> Result<Instance> {
        check_file_name(id)
            .map_err(|_| AppError::new("error.instance.not_found").with_param("id", id))?;
        let bytes = match std::fs::read(self.json_path(id)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::new("error.instance.not_found").with_param("id", id))
            }
            Err(e) => return Err(e.into()),
        };
        serde_json::from_slice(&bytes).map_err(|e| {
            AppError::new("error.instance.corrupt")
                .with_param("id", id)
                .with_detail(e.to_string())
        })
    }

    pub fn create(&self, req: NewInstance, default_memory_mb: u32) -> Result<Instance> {
        let base = slugify(&req.name);
        let mut id = base.clone();
        let mut n = 2;
        while self.paths.instance_dir(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let inst = Instance {
            schema: 1,
            id: id.clone(),
            name: req.name,
            minecraft: req.minecraft,
            loader: req.loader,
            java: JavaSetting::Auto,
            memory_mb: req.memory_mb.unwrap_or(default_memory_mb),
            jvm_args: Vec::new(),
            created_at: Utc::now(),
            last_played_at: None,
            mods: BTreeMap::new(),
        };
        for sub in GAME_SUBDIRS {
            std::fs::create_dir_all(self.game_dir(&id).join(sub))?;
        }
        self.save(&inst)?;
        Ok(inst)
    }

    pub fn save(&self, inst: &Instance) -> Result<()> {
        check_file_name(&inst.id)?;
        write_atomic(&self.json_path(&inst.id), &serde_json::to_vec_pretty(inst)?)
    }

    /// Never deletes: moves the whole folder to `trash/<id>-<timestamp>`.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.get(id)?;
        std::fs::create_dir_all(self.paths.trash_dir())?;
        let dest = self
            .paths
            .trash_dir()
            .join(format!("{id}-{}", Utc::now().format("%Y%m%d-%H%M%S%.3f")));
        std::fs::rename(self.paths.instance_dir(id), dest)?;
        Ok(())
    }

    pub fn list_mods(&self, id: &str) -> Result<Vec<ModFile>> {
        let dir = self.game_dir(id).join("mods");
        let mut out = Vec::new();
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let enabled = name.ends_with(".jar");
            if !(enabled || name.ends_with(".jar.disabled")) || !e.path().is_file() {
                continue;
            }
            out.push(ModFile {
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file_name: name,
                enabled,
            });
        }
        out.sort_by_key(|m| m.file_name.to_lowercase());
        Ok(out)
    }

    /// Enable (`x.jar`) or disable (`x.jar.disabled`) a mod file.
    pub fn set_mod_enabled(&self, id: &str, file_name: &str, enabled: bool) -> Result<()> {
        check_file_name(file_name)?;
        let dir = self.game_dir(id).join("mods");
        let base = file_name.strip_suffix(".disabled").unwrap_or(file_name);
        let (from, to) = if enabled {
            (format!("{base}.disabled"), base.to_string())
        } else {
            (base.to_string(), format!("{base}.disabled"))
        };
        if dir.join(&to).exists() && !dir.join(&from).exists() {
            return Ok(());
        }
        std::fs::rename(dir.join(&from), dir.join(&to)).map_err(|e| {
            AppError::new("error.instance.bad_file")
                .with_param("file", file_name)
                .with_detail(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, InstanceStore) {
        let d = tempfile::tempdir().unwrap();
        let s = InstanceStore::new(Paths::new(d.path().to_path_buf()));
        (d, s)
    }

    fn req(name: &str) -> NewInstance {
        NewInstance {
            name: name.into(),
            minecraft: "1.20.1".into(),
            loader: None,
            memory_mb: None,
        }
    }

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Survival 1.20.1"), "survival-1-20-1");
        assert_eq!(slugify("我的世界"), "instance");
        assert_eq!(slugify("  A--B  "), "a-b");
        assert_eq!(slugify("Créative"), "cr-ative");
    }

    #[test]
    fn create_duplicate_name_gets_suffix() {
        let (_d, s) = store();
        assert_eq!(s.create(req("Test"), 3072).unwrap().id, "test");
        assert_eq!(s.create(req("Test"), 3072).unwrap().id, "test-2");
        assert_eq!(s.create(req("test"), 3072).unwrap().id, "test-3");
    }

    #[test]
    fn create_with_cjk_name_gets_fallback_id() {
        let (_d, s) = store();
        let i = s.create(req("我的世界"), 3072).unwrap();
        assert_eq!((i.id.as_str(), i.name.as_str()), ("instance", "我的世界"));
        assert_eq!(s.create(req("生存"), 3072).unwrap().id, "instance-2");
    }

    #[test]
    fn create_makes_game_dirs_and_json_roundtrips() {
        let (_d, s) = store();
        let mut r = req("Fab");
        r.loader = Some(LoaderSpec {
            kind: LoaderKind::Fabric,
            version: "0.16.9".into(),
        });
        r.memory_mb = Some(4096);
        let i = s.create(r, 3072).unwrap();
        for sub in ["mods", "saves", "resourcepacks", "shaderpacks", "config"] {
            assert!(s.game_dir(&i.id).join(sub).is_dir(), "{sub}");
        }
        assert_eq!(i.memory_mb, 4096);
        assert_eq!(i.java, JavaSetting::Auto);
        assert_eq!(s.get(&i.id).unwrap(), i);
        let text =
            std::fs::read_to_string(s.paths().instance_dir(&i.id).join("instance.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["loader"]["kind"], "fabric");
        assert_eq!(v["java"]["mode"], "auto");
        assert_eq!(v["schema"], 1);
    }

    #[test]
    fn default_memory_used_when_not_given() {
        let (_d, s) = store();
        assert_eq!(s.create(req("M"), 6144).unwrap().memory_mb, 6144);
    }

    #[test]
    fn list_skips_corrupt_instance() {
        let (_d, s) = store();
        s.create(req("Good"), 3072).unwrap();
        let bad = s.paths().instance_dir("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("instance.json"), "oops").unwrap();
        let (list, errors) = s.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].key, "error.instance.corrupt");
        assert_eq!(errors[0].params.get("id").map(String::as_str), Some("bad"));
    }

    #[test]
    fn get_missing_instance_errors() {
        let (_d, s) = store();
        assert_eq!(s.get("nope").unwrap_err().key, "error.instance.not_found");
    }

    #[test]
    fn toggle_mod_renames_file() {
        let (_d, s) = store();
        let i = s.create(req("Mods"), 3072).unwrap();
        let mods = s.game_dir(&i.id).join("mods");
        std::fs::write(mods.join("b.jar"), b"12345").unwrap();
        std::fs::write(mods.join("a.jar"), b"1").unwrap();
        std::fs::write(mods.join("notes.txt"), b"x").unwrap();
        let list = s.list_mods(&i.id).unwrap();
        assert_eq!(
            list.iter()
                .map(|m| m.file_name.as_str())
                .collect::<Vec<_>>(),
            ["a.jar", "b.jar"]
        );
        assert_eq!(list[1].size, 5);
        s.set_mod_enabled(&i.id, "a.jar", false).unwrap();
        assert!(mods.join("a.jar.disabled").exists());
        let list = s.list_mods(&i.id).unwrap();
        assert_eq!(
            (list[0].file_name.as_str(), list[0].enabled),
            ("a.jar.disabled", false)
        );
        s.set_mod_enabled(&i.id, "a.jar.disabled", true).unwrap();
        assert!(mods.join("a.jar").exists());
    }

    #[test]
    fn toggle_rejects_path_escape() {
        let (_d, s) = store();
        let i = s.create(req("Esc"), 3072).unwrap();
        assert_eq!(
            s.set_mod_enabled(&i.id, "../x.jar", false).unwrap_err().key,
            "error.instance.bad_file"
        );
    }

    #[test]
    fn delete_moves_to_trash() {
        let (_d, s) = store();
        let i = s.create(req("Del"), 3072).unwrap();
        s.delete(&i.id).unwrap();
        assert!(!s.paths().instance_dir(&i.id).exists());
        let trashed: Vec<_> = std::fs::read_dir(s.paths().trash_dir()).unwrap().collect();
        assert_eq!(trashed.len(), 1);
        assert!(s.list().unwrap().0.is_empty());
    }

    #[test]
    fn save_updates_fields() {
        let (_d, s) = store();
        let mut i = s.create(req("Save"), 3072).unwrap();
        i.memory_mb = 8192;
        i.jvm_args = vec!["-XX:+UseG1GC".into()];
        s.save(&i).unwrap();
        assert_eq!(s.get(&i.id).unwrap().jvm_args, ["-XX:+UseG1GC"]);
    }
}
