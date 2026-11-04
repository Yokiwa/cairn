//! Java runtimes: find installed ones, pick one, download Temurin if needed.

pub mod adoptium;
pub mod detect;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::versions::rules::Environment;
use crate::versions::version_json::VersionJson;

pub use adoptium::{download_java, ensure_java};
pub use detect::find_installations;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct JavaInstall {
    /// The `java` executable.
    #[ts(type = "string")]
    pub path: PathBuf,
    pub major: u32,
    pub version: String,
    /// "x86_64", "arm64" or "x86".
    pub arch: String,
    /// Downloaded by Cairn into its own `java/` directory.
    pub managed: bool,
}

/// "1.8.0_392" → 8, "17.0.9" → 17, "21" → 21.
pub fn parse_version(s: &str) -> Option<u32> {
    let mut parts = s
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty());
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// The Java major version a game version needs (8 when unspecified).
pub fn required_major(v: &VersionJson) -> u32 {
    v.java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(8)
}

/// Pick an installed runtime: exact major; same arch first, then Cairn-managed.
pub fn select(installs: &[JavaInstall], major: u32) -> Option<&JavaInstall> {
    let arch = Environment::current().arch;
    installs
        .iter()
        .filter(|j| j.major == major)
        .max_by_key(|j| (j.arch == arch, j.managed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ji(major: u32, arch: &str, managed: bool) -> JavaInstall {
        JavaInstall {
            path: PathBuf::from(format!("/j/{major}-{arch}-{managed}")),
            major,
            version: format!("{major}.0.1"),
            arch: arch.into(),
            managed,
        }
    }

    #[test]
    fn parse_version_forms() {
        assert_eq!(parse_version("1.8.0_392"), Some(8));
        assert_eq!(parse_version("17.0.9"), Some(17));
        assert_eq!(parse_version("21"), Some(21));
        assert_eq!(parse_version("25-ea"), Some(25));
        assert_eq!(parse_version("garbage"), None);
    }

    #[test]
    fn required_major_defaults_to_8() {
        let read = |n: &str| -> crate::versions::version_json::VersionJson {
            serde_json::from_str(
                &std::fs::read_to_string(format!(
                    "{}/tests/fixtures/versions/{n}",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .unwrap(),
            )
            .unwrap()
        };
        let mut old = read("1.8.9.json");
        assert_eq!(required_major(&old), 8);
        old.java_version = None;
        assert_eq!(required_major(&old), 8);
        assert_eq!(required_major(&read("1.20.1.json")), 17);
    }

    #[test]
    fn select_requires_exact_major_and_prefers_managed() {
        let here = crate::versions::rules::Environment::current().arch;
        let list = vec![
            ji(21, &here, false),
            ji(17, &here, false),
            ji(17, &here, true),
            ji(8, &here, false),
        ];
        let got = select(&list, 17).unwrap();
        assert_eq!((got.major, got.managed), (17, true));
        assert!(select(&list, 16).is_none());
    }

    #[test]
    fn select_prefers_matching_arch() {
        let here = crate::versions::rules::Environment::current().arch;
        let other = if here == "arm64" { "x86_64" } else { "arm64" };
        let list = vec![ji(17, other, true), ji(17, &here, false)];
        assert_eq!(select(&list, 17).unwrap().arch, here);
    }
}
