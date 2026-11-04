//! Forge and NeoForge: download the official installer and do what it would
//! do for a client — write the version profile, unpack bundled libraries,
//! download the rest and run the client-side processors.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::LoaderVersion;
use crate::download::{sha1_file, DownloadItem, ProgressFn};
use crate::fsutil::write_atomic;
use crate::instances::{LoaderKind, LoaderSpec};
use crate::java::{ensure_java, required_major};
use crate::versions::install::{
    client_jar_path, install_version, plan_game_files, version_json_path,
};
use crate::versions::maven::maven_path;
use crate::versions::rules::Environment;
use crate::versions::version_json::{Library, VersionJson};
use crate::{AppError, Core, Result};

// ---------- version lists ----------

/// NeoForge version → Minecraft version.
/// Old scheme: "21.1.77" → "1.21.1", "21.0.3" → "1.21".
/// Year scheme: "26.3.0.101" → "26.3", "26.1.2.5" → "26.1.2".
pub fn neoforge_mc(v: &str) -> Option<String> {
    let core = v.split('-').next()?;
    let parts: Vec<&str> = core.split('.').collect();
    let nums: Vec<u32> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match nums.as_slice() {
        [0, ..] => None,
        [a, b, c, _] if *a >= 26 => Some(if *c == 0 {
            format!("{a}.{b}")
        } else {
            format!("{a}.{b}.{c}")
        }),
        [a, b, _] => Some(if *b == 0 {
            format!("1.{a}")
        } else {
            format!("1.{a}.{b}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

async fn get_text(core: &Core, url: &str) -> Result<String> {
    let bytes = core.http.get_bytes(url).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub async fn list_versions(core: &Core, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    match kind {
        LoaderKind::NeoForge if mc == "1.20.1" => {
            // NeoForge's 1.20.1 builds were published as net.neoforged:forge.
            let url = format!(
                "{}/api/maven/versions/releases/net/neoforged/forge",
                core.endpoints.neoforge_maven
            );
            let list: NeoVersions = core.http.get_json(&url).await?;
            Ok(list
                .versions
                .into_iter()
                .filter_map(|v| v.strip_prefix("1.20.1-").map(str::to_string))
                .map(|v| LoaderVersion {
                    stable: !v.contains("beta"),
                    version: v,
                })
                .collect())
        }
        LoaderKind::NeoForge => {
            let url = format!(
                "{}/api/maven/versions/releases/net/neoforged/neoforge",
                core.endpoints.neoforge_maven
            );
            let list: NeoVersions = core.http.get_json(&url).await?;
            Ok(list
                .versions
                .into_iter()
                .filter(|v| neoforge_mc(v).as_deref() == Some(mc))
                .map(|v| LoaderVersion {
                    stable: !v.contains("beta"),
                    version: v,
                })
                .collect())
        }
        _ => {
            let xml = get_text(
                core,
                &format!(
                    "{}/net/minecraftforge/forge/maven-metadata.xml",
                    core.endpoints.forge_maven
                ),
            )
            .await?;
            let recommended = core
                .http
                .get_json::<Promotions>(&format!(
                    "{}/net/minecraftforge/forge/promotions_slim.json",
                    core.endpoints.forge_files
                ))
                .await
                .ok()
                .and_then(|p| p.promos.get(&format!("{mc}-recommended")).cloned());
            let prefix = format!("{mc}-");
            let re = regex::Regex::new(r"<version>([^<]+)</version>").expect("static regex");
            Ok(re
                .captures_iter(&xml)
                .filter_map(|c| c[1].strip_prefix(&prefix).map(str::to_string))
                .map(|v| LoaderVersion {
                    stable: recommended.as_deref() == Some(v.as_str()),
                    version: v,
                })
                .collect())
        }
    }
}

// ---------- installer ----------

fn installer_url(core: &Core, kind: LoaderKind, mc: &str, v: &str) -> (String, String) {
    match kind {
        LoaderKind::NeoForge if mc == "1.20.1" => {
            let mv = format!("1.20.1-{v}");
            (
                format!(
                    "{}/releases/net/neoforged/forge/{mv}/forge-{mv}-installer.jar",
                    core.endpoints.neoforge_maven
                ),
                mv,
            )
        }
        LoaderKind::NeoForge => (
            format!(
                "{}/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
                core.endpoints.neoforge_maven
            ),
            v.to_string(),
        ),
        _ => {
            let mv = format!("{mc}-{v}");
            (
                format!(
                    "{}/net/minecraftforge/forge/{mv}/forge-{mv}-installer.jar",
                    core.endpoints.forge_maven
                ),
                mv,
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct SidedValue {
    client: String,
}

#[derive(Debug, Deserialize)]
struct Processor {
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: Option<HashMap<String, String>>,
    #[serde(default)]
    sides: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct NewProfile {
    json: String,
    #[serde(default)]
    data: HashMap<String, SidedValue>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyInstall {
    path: String,
    file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProfile {
    install: LegacyInstall,
    version_info: VersionJson,
}

fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::new("error.loader.bad_installer").with_detail(e.to_string())
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>> {
    let mut f = zip.by_name(name.trim_start_matches('/')).map_err(zip_err)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, dest: &Path) -> Result<()> {
    let bytes = read_entry(zip, name)?;
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(dest, bytes)?;
    Ok(())
}

/// Unpack the installer's bundled `maven/` repository into the libraries dir.
fn extract_bundled_maven(zip: &mut zip::ZipArchive<std::fs::File>, libraries: &Path) -> Result<()> {
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(zip_err)?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let Ok(inside) = rel.strip_prefix("maven") else {
            continue;
        };
        let dest = libraries.join(inside);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}

fn libs_only(libraries: Vec<Library>) -> VersionJson {
    VersionJson {
        id: String::new(),
        inherits_from: None,
        main_class: String::new(),
        arguments: None,
        minecraft_arguments: None,
        libraries,
        downloads: None,
        asset_index: None,
        assets: None,
        java_version: None,
        logging: None,
        kind: None,
        jar: None,
    }
}

fn main_class_of(jar: &Path) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar)?).map_err(zip_err)?;
    let manifest =
        String::from_utf8_lossy(&read_entry(&mut zip, "META-INF/MANIFEST.MF")?).into_owned();
    manifest
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|v| v.trim().to_string()))
        .ok_or_else(|| {
            AppError::new("error.loader.bad_installer")
                .with_detail(format!("no Main-Class in {}", jar.display()))
        })
}

struct Ctx<'a> {
    vars: HashMap<String, String>,
    libraries: &'a Path,
}

impl Ctx<'_> {
    fn lib(&self, coord: &str) -> Result<String> {
        Ok(self
            .libraries
            .join(maven_path(coord)?)
            .to_string_lossy()
            .into_owned())
    }

    /// `{KEY}` → variable, `[coord]` → library path, `'text'` → text.
    fn resolve(&self, s: &str) -> Result<String> {
        if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            return self.lib(inner);
        }
        if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
            return Ok(inner.to_string());
        }
        let mut out = s.to_string();
        for (k, v) in &self.vars {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        Ok(out)
    }
}

#[allow(clippy::too_many_arguments)] // internal step of `install`; the arguments are its locals
async fn run_processors(
    core: &Core,
    profile: &NewProfile,
    zip: &mut zip::ZipArchive<std::fs::File>,
    installer: &Path,
    work: &Path,
    mc: &str,
    vanilla: &VersionJson,
    progress: &ProgressFn,
    cancel: &CancellationToken,
) -> Result<()> {
    let libraries = core.paths.libraries_dir();
    let mut ctx = Ctx {
        vars: HashMap::new(),
        libraries: &libraries,
    };
    ctx.vars.insert("SIDE".into(), "client".into());
    ctx.vars.insert(
        "MINECRAFT_JAR".into(),
        client_jar_path(&core.paths, vanilla)
            .to_string_lossy()
            .into_owned(),
    );
    ctx.vars.insert("MINECRAFT_VERSION".into(), mc.into());
    ctx.vars.insert(
        "ROOT".into(),
        core.paths.root().to_string_lossy().into_owned(),
    );
    ctx.vars
        .insert("INSTALLER".into(), installer.to_string_lossy().into_owned());
    ctx.vars.insert(
        "LIBRARY_DIR".into(),
        libraries.to_string_lossy().into_owned(),
    );
    for (k, v) in &profile.data {
        let value = if v.client.starts_with('/') {
            let dest = work.join(v.client.trim_start_matches('/'));
            extract_entry(zip, &v.client, &dest)?;
            dest.to_string_lossy().into_owned()
        } else {
            ctx.resolve(&v.client)?
        };
        ctx.vars.insert(k.clone(), value);
    }

    let client: Vec<&Processor> = profile
        .processors
        .iter()
        .filter(|p| {
            p.sides
                .as_ref()
                .is_none_or(|s| s.iter().any(|x| x == "client"))
        })
        .collect();
    if client.is_empty() {
        return Ok(());
    }
    let java = ensure_java(
        core,
        required_major(vanilla),
        progress.clone(),
        cancel.clone(),
    )
    .await?;
    let sep = if cfg!(windows) { ";" } else { ":" };

    for p in client {
        if cancel.is_cancelled() {
            return Err(crate::tasks::cancelled());
        }
        let outputs: Vec<(String, String)> = match &p.outputs {
            Some(o) => o
                .iter()
                .map(|(k, v)| Ok((ctx.resolve(k)?, ctx.resolve(v)?)))
                .collect::<Result<_>>()?,
            None => Vec::new(),
        };
        let outputs_ok = |outs: &[(String, String)]| {
            outs.iter().all(|(file, sha)| {
                sha1_file(Path::new(file))
                    .map(|got| got.eq_ignore_ascii_case(sha))
                    .unwrap_or(false)
            })
        };
        if !outputs.is_empty() && outputs_ok(&outputs) {
            continue;
        }
        let jar = PathBuf::from(ctx.lib(&p.jar)?);
        let mut cp = vec![jar.to_string_lossy().into_owned()];
        for c in &p.classpath {
            cp.push(ctx.lib(c)?);
        }
        let main = main_class_of(&jar)?;
        let args: Vec<String> = p
            .args
            .iter()
            .map(|a| ctx.resolve(a))
            .collect::<Result<_>>()?;
        let out = tokio::process::Command::new(&java.path)
            .arg("-cp")
            .arg(cp.join(sep))
            .arg(&main)
            .args(&args)
            .current_dir(work)
            .output()
            .await?;
        if !out.status.success() {
            let text = String::from_utf8_lossy(&out.stderr).to_string()
                + &String::from_utf8_lossy(&out.stdout);
            let tail: Vec<&str> = text
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Err(AppError::new("error.loader.processor_failed")
                .with_param("processor", p.jar.clone())
                .with_detail(format!("exit {:?}\n{}", out.status.code(), tail.join("\n"))));
        }
        if !outputs.is_empty() && !outputs_ok(&outputs) {
            return Err(AppError::new("error.loader.processor_output")
                .with_param("processor", p.jar.clone()));
        }
    }
    Ok(())
}

/// Install Forge or NeoForge for `mc`; returns the version id to launch.
pub async fn install(
    core: &Core,
    mc: &str,
    loader: &LoaderSpec,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<String> {
    // Processors patch the vanilla client jar, so vanilla must be complete first.
    let vanilla = install_version(core, mc, progress.clone(), cancel.clone()).await?;

    let (url, mv) = installer_url(core, loader.kind, mc, &loader.version);
    let installer = core
        .paths
        .cache_dir()
        .join("installers")
        .join(format!("{mv}-installer.jar"));
    let _ = std::fs::remove_file(&installer);
    core.downloader
        .download_all(
            "loader",
            vec![DownloadItem {
                url,
                dest: installer.clone(),
                sha1: None,
                size: None,
            }],
            progress.clone(),
            cancel.clone(),
        )
        .await
        .map_err(|e| {
            if e.key == "error.http" {
                AppError::new("error.loader.not_found")
                    .with_param("loader", format!("{:?} {}", loader.kind, loader.version))
                    .with_param("mc", mc)
            } else {
                e
            }
        })?;

    let mut zip = zip::ZipArchive::new(std::fs::File::open(&installer)?).map_err(zip_err)?;
    let raw: serde_json::Value =
        serde_json::from_slice(&read_entry(&mut zip, "install_profile.json")?)?;
    let env = Environment::current();

    if raw.get("versionInfo").is_some() {
        // Pre-2018 format (old 1.12.2 builds): profile inline, universal jar at the root.
        let legacy: LegacyProfile = serde_json::from_value(raw)?;
        let id = legacy.version_info.id.clone();
        let dest = core
            .paths
            .libraries_dir()
            .join(maven_path(&legacy.install.path)?);
        extract_entry(&mut zip, &legacy.install.file_path, &dest)?;
        write_atomic(
            &version_json_path(&core.paths, &id),
            &serde_json::to_vec_pretty(&legacy.version_info)?,
        )?;
        return Ok(id);
    }

    let profile: NewProfile = serde_json::from_value(raw)?;
    let version: VersionJson = serde_json::from_slice(&read_entry(&mut zip, &profile.json)?)?;
    let id = version.id.clone();

    extract_bundled_maven(&mut zip, &core.paths.libraries_dir())?;
    let mut items = plan_game_files(
        &libs_only(profile.libraries.clone()),
        &env,
        &core.paths,
        &core.endpoints,
    )?;
    items.extend(plan_game_files(
        &libs_only(version.libraries.clone()),
        &env,
        &core.paths,
        &core.endpoints,
    )?);
    core.downloader
        .download_all("loader", items, progress.clone(), cancel.clone())
        .await?;

    let work = core
        .paths
        .cache_dir()
        .join("installers")
        .join(format!("{mv}-work"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let result = run_processors(
        core, &profile, &mut zip, &installer, &work, mc, &vanilla, &progress, &cancel,
    )
    .await;
    let _ = std::fs::remove_dir_all(&work);
    result?;

    write_atomic(
        &version_json_path(&core.paths, &id),
        &serde_json::to_vec_pretty(&version)?,
    )?;
    let _ = std::fs::remove_file(&installer);
    Ok(id)
}
