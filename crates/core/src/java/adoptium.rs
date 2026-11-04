//! Download Eclipse Temurin JREs from the Adoptium API.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use super::detect::{inspect_home, java_exe_name};
use super::{select, JavaInstall};
use crate::download::{DownloadItem, ProgressFn};
use crate::versions::rules::{Environment, OsName};
use crate::{AppError, Core, Result};

#[derive(Debug, Deserialize)]
struct Asset {
    binary: Binary,
}
#[derive(Debug, Deserialize)]
struct Binary {
    package: Package,
}
#[derive(Debug, Deserialize)]
struct Package {
    link: String,
    checksum: String,
    name: String,
    size: Option<u64>,
}

fn adoptium_os() -> &'static str {
    match OsName::current() {
        OsName::Windows => "windows",
        OsName::Osx => "mac",
        _ => "linux",
    }
}

fn adoptium_arch(arch: &str) -> &'static str {
    match arch {
        "arm64" => "aarch64",
        "x86" => "x86",
        _ => "x64",
    }
}

async fn find_package(core: &Core, major: u32, arch: &str) -> Result<Option<Package>> {
    let url = format!(
        "{}/v3/assets/latest/{major}/hotspot?architecture={}&image_type=jre&os={}&vendor=eclipse",
        core.endpoints.adoptium,
        adoptium_arch(arch),
        adoptium_os()
    );
    let assets: Vec<Asset> = core.http.get_json(&url).await?;
    Ok(assets.into_iter().next().map(|a| a.binary.package))
}

fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

/// Unpack a .zip or .tar.gz into `dest`, refusing entries that escape it.
pub fn extract_archive(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)
            .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))?;
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))?;
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let out = dest.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut f = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut f)?;
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
            }
        }
    } else {
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
        let mut tar = tar::Archive::new(gz);
        for entry in tar.entries()? {
            let mut entry = entry?;
            // unpack_in refuses paths that would escape `dest`.
            entry.unpack_in(dest)?;
        }
    }
    Ok(())
}

/// Find the Java home inside an unpacked archive.
fn find_home(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if dir.join("bin").join(java_exe_name()).is_file() {
            return Some(dir);
        }
        if depth >= 4 {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    stack.push((e.path(), depth + 1));
                }
            }
        }
    }
    None
}

/// Download Temurin `major` into `java/temurin-<major>/`, whatever is installed.
pub async fn download_java(
    core: &Core,
    major: u32,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<JavaInstall> {
    let arch = Environment::current().arch;
    let mut pkg = find_package(core, major, &arch).await?;
    // Apple silicon: old Javas have no aarch64 build; Rosetta runs the x64 one.
    if pkg.is_none() && OsName::current() == OsName::Osx && arch == "arm64" {
        pkg = find_package(core, major, "x86_64").await?;
    }
    let pkg = pkg.ok_or_else(|| {
        AppError::new("error.java.unavailable").with_param("major", major.to_string())
    })?;

    let archive = core.paths.cache_dir().join("java").join(&pkg.name);
    let _ = std::fs::remove_file(&archive);
    core.downloader
        .download_all(
            "java",
            vec![DownloadItem {
                url: pkg.link.clone(),
                dest: archive.clone(),
                sha1: None,
                size: pkg.size,
            }],
            progress,
            cancel,
        )
        .await?;

    let got = sha256_file(&archive)?;
    if !got.eq_ignore_ascii_case(&pkg.checksum) {
        let _ = std::fs::remove_file(&archive);
        return Err(AppError::new("error.java.checksum")
            .with_detail(format!("expected {}, got {got}", pkg.checksum)));
    }

    let final_dir = core.paths.java_dir().join(format!("temurin-{major}"));
    let tmp_dir = core.paths.java_dir().join(format!(".temurin-{major}.tmp"));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let unpack = {
        let (archive, tmp_dir) = (archive.clone(), tmp_dir.clone());
        tokio::task::spawn_blocking(move || extract_archive(&archive, &tmp_dir))
    };
    unpack
        .await
        .map_err(|e| AppError::new("error.io").with_detail(e.to_string()))??;
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&tmp_dir, &final_dir)?;
    let _ = std::fs::remove_file(&archive);

    let home = find_home(&final_dir).ok_or_else(|| {
        AppError::new("error.java.invalid_path")
            .with_detail("no bin/java in the downloaded archive")
    })?;
    inspect_home(&home, &core.paths.java_dir()).ok_or_else(|| {
        AppError::new("error.java.invalid_path").with_detail(home.display().to_string())
    })
}

/// An installed runtime of exactly `major`, downloading one if none exists.
pub async fn ensure_java(
    core: &Core,
    major: u32,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<JavaInstall> {
    let installs = super::find_installations(&core.paths);
    if let Some(j) = select(&installs, major) {
        return Ok(j.clone());
    }
    download_java(core, major, progress, cancel).await
}
