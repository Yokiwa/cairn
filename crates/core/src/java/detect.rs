use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{parse_version, JavaInstall};
use crate::Paths;

pub fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn normalize_arch(a: &str) -> String {
    match a {
        "amd64" | "x86_64" | "x64" => "x86_64".into(),
        "aarch64" | "arm64" => "arm64".into(),
        "x86" | "i386" | "i686" => "x86".into(),
        other => other.into(),
    }
}

/// `JAVA_VERSION` and `OS_ARCH` from a JDK/JRE `release` file.
pub fn parse_release(text: &str) -> Option<(String, Option<String>)> {
    let get = |key: &str| {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
        })
    };
    Some((
        get("JAVA_VERSION")?,
        get("OS_ARCH").map(|a| normalize_arch(&a)),
    ))
}

/// The quoted version in `java -version` output.
pub fn parse_version_output(out: &str) -> Option<String> {
    let start = out.find("version \"")? + "version \"".len();
    let rest = &out[start..];
    Some(rest[..rest.find('"')?].to_string())
}

/// Candidate Java homes (directories containing `bin/java`).
fn candidate_homes(paths: &Paths) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        homes.push(PathBuf::from(h));
    }
    if let Some(p) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&p) {
            let exe = dir.join(java_exe_name());
            if exe.is_file() {
                let real = std::fs::canonicalize(&exe).unwrap_or(exe);
                if let Some(home) = real.parent().and_then(Path::parent) {
                    homes.push(home.to_path_buf());
                }
            }
        }
    }
    let mut roots: Vec<PathBuf> = vec![paths.java_dir()];
    if cfg!(windows) {
        for base in ["C:\\Program Files", "C:\\Program Files (x86)"] {
            for vendor in [
                "Java",
                "Eclipse Adoptium",
                "Microsoft",
                "Zulu",
                "BellSoft",
                "Amazon Corretto",
            ] {
                roots.push(PathBuf::from(base).join(vendor));
            }
        }
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        if let Some(home) = directories::BaseDirs::new() {
            roots.push(home.home_dir().join("Library/Java/JavaVirtualMachines"));
        }
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/java"));
        roots.push(PathBuf::from("/opt/java"));
    }
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            homes.push(p.join("Contents").join("Home"));
            homes.push(p.clone());
            // Archives unpack into one more directory level (temurin-17/jdk-17…).
            if let Ok(inner) = std::fs::read_dir(&p) {
                for i in inner.flatten() {
                    homes.push(i.path().join("Contents").join("Home"));
                    homes.push(i.path());
                }
            }
        }
    }
    homes
}

/// Inspect one Java home. Reads `release`; falls back to running `java -version`.
pub fn inspect_home(home: &Path, managed_root: &Path) -> Option<JavaInstall> {
    let exe = home.join("bin").join(java_exe_name());
    if !exe.is_file() {
        return None;
    }
    let (version, arch) = match std::fs::read_to_string(home.join("release"))
        .ok()
        .and_then(|t| parse_release(&t))
    {
        Some((v, a)) => (v, a),
        None => {
            let out = std::process::Command::new(&exe)
                .arg("-version")
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&out.stderr).to_string()
                + &String::from_utf8_lossy(&out.stdout);
            (parse_version_output(&text)?, None)
        }
    };
    let major = parse_version(&version)?;
    Some(JavaInstall {
        managed: exe.starts_with(managed_root),
        path: exe,
        major,
        version,
        arch: arch.unwrap_or_else(|| crate::versions::rules::Environment::current().arch),
    })
}

/// Every Java runtime we can find, de-duplicated by executable path.
pub fn find_installations(paths: &Paths) -> Vec<JavaInstall> {
    let mut seen = HashSet::new();
    let managed_root = paths.java_dir();
    candidate_homes(paths)
        .into_iter()
        .filter_map(|h| inspect_home(&h, &managed_root))
        .filter(|j| seen.insert(std::fs::canonicalize(&j.path).unwrap_or_else(|_| j.path.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_release_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::Paths::new(dir.path().to_path_buf());
        let home = paths.java_dir().join("temurin-17");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(
            home.join("release"),
            "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"17.0.9\"\nOS_ARCH=\"x86_64\"\n",
        )
        .unwrap();
        std::fs::write(home.join("bin").join(java_exe_name()), b"").unwrap();
        let found = find_installations(&paths);
        let j = found
            .iter()
            .find(|j| j.path.starts_with(&home))
            .expect("managed java found");
        assert_eq!(
            (j.major, j.version.as_str(), j.arch.as_str(), j.managed),
            (17, "17.0.9", "x86_64", true)
        );
    }

    #[test]
    fn parses_release_text() {
        let (v, arch) = parse_release("JAVA_VERSION=\"1.8.0_392\"\nOS_ARCH=\"aarch64\"").unwrap();
        assert_eq!((v.as_str(), arch.as_deref()), ("1.8.0_392", Some("arm64")));
    }

    #[test]
    fn parses_java_version_output() {
        let out = "openjdk version \"21.0.2\" 2024-01-16\nOpenJDK Runtime Environment";
        assert_eq!(parse_version_output(out).as_deref(), Some("21.0.2"));
    }
}
