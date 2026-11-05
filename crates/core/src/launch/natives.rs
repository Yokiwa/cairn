//! Native libraries and legacy asset layouts, prepared before each launch.

use std::path::Path;

use crate::versions::install::{applicable_libraries, natives_path, AssetIndex};
use crate::versions::rules::Environment;
use crate::versions::version_json::VersionJson;
use crate::{AppError, Paths, Result};

/// Unpack old-style native jars (libraries with a `natives` map) into `dest`,
/// emptying it first. Entries under an `extract.exclude` prefix (by default
/// `META-INF/`) are skipped.
pub fn extract_natives(
    v: &VersionJson,
    env: &Environment,
    paths: &Paths,
    dest: &Path,
) -> Result<()> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest)?;
    for lib in applicable_libraries(v, env) {
        let Some(jar) = natives_path(paths, lib, env)? else {
            continue;
        };
        if !jar.is_file() {
            continue;
        }
        let excludes: Vec<String> = lib
            .extract
            .as_ref()
            .map(|e| e.exclude.clone())
            .unwrap_or_else(|| vec!["META-INF/".into()]);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?).map_err(|e| {
            AppError::new("error.parse").with_detail(format!("{}: {e}", jar.display()))
        })?;
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))?;
            let name = entry.name().to_string();
            if entry.is_dir() || excludes.iter().any(|x| name.starts_with(x.as_str())) {
                continue;
            }
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let out = dest.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut f = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut f)?;
        }
    }
    Ok(())
}

/// Copy hashed assets to readable names for versions that need it
/// (`virtual` / `map_to_resources` indexes, i.e. 1.7.2 and older).
pub fn prepare_legacy_assets(index: &AssetIndex, paths: &Paths, target: &Path) -> Result<()> {
    if !(index.is_virtual || index.map_to_resources) {
        return Ok(());
    }
    for (name, obj) in &index.objects {
        let src = paths
            .assets_dir()
            .join("objects")
            .join(&obj.hash[..2.min(obj.hash.len())])
            .join(&obj.hash);
        let dst = target.join(name);
        if dst.is_file() || !src.is_file() {
            continue;
        }
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(&src, &dst)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extract_natives_skips_meta_inf_and_clears_dest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().to_path_buf());
        let v: VersionJson = serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/tests/fixtures/versions/1.8.9.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let env = Environment {
            os: crate::versions::rules::OsName::Linux,
            arch: "x86_64".into(),
            os_version: String::new(),
            features: Default::default(),
        };
        for lib in v.libraries.iter().filter(|l| l.natives.is_some()) {
            let p = crate::versions::install::natives_path(&paths, lib, &env)
                .unwrap()
                .unwrap();
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file(format!("lib{}.so", lib.name.len()), o)
                .unwrap();
            z.write_all(b"so").unwrap();
            z.start_file("META-INF/MANIFEST.MF", o).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        let dest = dir.path().join("natives");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("stale.so"), b"old").unwrap();
        extract_natives(&v, &env, &paths, &dest).unwrap();
        let names: Vec<String> = std::fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.iter().all(|n| n.ends_with(".so")));
        assert!(!dest.join("META-INF").exists());
        assert!(!dest.join("stale.so").exists());
    }

    #[test]
    fn legacy_assets_copied_to_names_only_for_virtual_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().to_path_buf());
        let hash = "ab12cd";
        let obj = paths.assets_dir().join("objects/ab").join(hash);
        std::fs::create_dir_all(obj.parent().unwrap()).unwrap();
        std::fs::write(&obj, b"ogg").unwrap();
        let mut index: AssetIndex = serde_json::from_value(
            serde_json::json!({"objects": {"sound/a.ogg": {"hash": hash, "size": 3}}}),
        )
        .unwrap();
        let target = dir.path().join("virtual");
        prepare_legacy_assets(&index, &paths, &target).unwrap();
        assert!(
            !target.join("sound/a.ogg").exists(),
            "modern index: nothing to do"
        );
        index.is_virtual = true;
        prepare_legacy_assets(&index, &paths, &target).unwrap();
        assert_eq!(std::fs::read(target.join("sound/a.ogg")).unwrap(), b"ogg");
    }
}
