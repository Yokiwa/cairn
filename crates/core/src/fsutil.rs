use std::io::Write;
use std::path::Path;

use crate::Result;

/// Write `bytes` to `path` so readers never see a half-written file:
/// write a temp file in the same directory, flush it to disk, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_replaces_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.json");
        write_atomic(&p, b"a much longer first version").unwrap();
        write_atomic(&p, b"short").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"short");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "leftover temp files: {names:?}");
    }

    #[test]
    fn write_atomic_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x/y/z.json");
        write_atomic(&p, b"{}").unwrap();
        assert!(p.exists());
    }
}
