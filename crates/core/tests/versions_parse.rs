use std::collections::HashMap;

use cairn_core::versions::compare_versions;
use cairn_core::versions::manifest::{VersionKind, VersionManifest};
use cairn_core::versions::maven::maven_path;
use cairn_core::versions::rules::{rules_allow, Environment, OsName, Rule};
use cairn_core::versions::version_json::{merge, Arg, VersionJson};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/versions/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn version(name: &str) -> VersionJson {
    serde_json::from_str(&fixture(name)).unwrap()
}
fn env(os: OsName) -> Environment {
    Environment {
        os,
        arch: "x86_64".into(),
        os_version: "10.0".into(),
        features: HashMap::new(),
    }
}
fn rules(json: &str) -> Vec<Rule> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn parses_manifest_kinds() {
    let m: VersionManifest = serde_json::from_str(&fixture("manifest.json")).unwrap();
    let kind = |id: &str| m.versions.iter().find(|v| v.id == id).unwrap().kind;
    assert_eq!(kind("1.20.1"), VersionKind::Release);
    assert_eq!(kind(&m.latest.snapshot), VersionKind::Snapshot);
    assert_eq!(kind("b1.7.3"), VersionKind::OldBeta);
    assert_eq!(kind("a1.0.4"), VersionKind::OldAlpha);
}

#[test]
fn parses_modern_and_legacy_args() {
    let modern = version("1.20.1.json");
    let args = modern.arguments.as_ref().unwrap();
    assert!(args.game.iter().any(|a| matches!(a, Arg::Conditional { rules, .. }
        if rules.iter().any(|r| r.features.as_ref().is_some_and(|f| f.contains_key("is_demo_user"))))));
    assert!(modern.minecraft_arguments.is_none());
    assert_eq!(modern.java_version.as_ref().unwrap().major_version, 17);
    let legacy = version("1.8.9.json");
    assert!(legacy.minecraft_arguments.is_some());
    assert!(legacy.arguments.is_none());
    assert!(legacy.libraries.iter().any(|l| l.natives.is_some()));
}

#[test]
fn rules_os_disallow_and_allow() {
    let r = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
    assert!(!rules_allow(&r, &env(OsName::Osx)));
    assert!(rules_allow(&r, &env(OsName::Linux)));
    let only_win = rules(r#"[{"action":"allow","os":{"name":"windows"}}]"#);
    assert!(rules_allow(&only_win, &env(OsName::Windows)));
    assert!(!rules_allow(&only_win, &env(OsName::Linux)));
    assert!(rules_allow(&[], &env(OsName::Linux)));
}

#[test]
fn rules_arch_and_version_regex() {
    let r = rules(r#"[{"action":"allow","os":{"name":"windows","arch":"x86"}}]"#);
    assert!(!rules_allow(&r, &env(OsName::Windows)));
    let v = rules(r#"[{"action":"allow","os":{"name":"osx","version":"^10\\.5\\.\\d$"}}]"#);
    let mut e = env(OsName::Osx);
    e.os_version = "10.5.8".into();
    assert!(rules_allow(&v, &e));
    e.os_version = "14.1".into();
    assert!(!rules_allow(&v, &e));
}

#[test]
fn rules_feature_flag() {
    let r = rules(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
    let mut e = env(OsName::Linux);
    assert!(!rules_allow(&r, &e));
    e.features.insert("has_custom_resolution".into(), true);
    assert!(rules_allow(&r, &e));
}

#[test]
fn maven_path_variants() {
    assert_eq!(
        maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(),
        "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
    );
    assert_eq!(
        maven_path("org.lwjgl:lwjgl:3.3.1:natives-linux").unwrap(),
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar"
    );
    assert_eq!(
        maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(),
        "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip"
    );
    assert_eq!(maven_path("a:b:1:c@txt").unwrap(), "a/b/1/b-1-c.txt");
    assert_eq!(maven_path("nonsense").unwrap_err().key, "error.parse");
}

#[test]
fn merge_fabric_over_vanilla() {
    let parent = version("1.20.1.json");
    let child = version("fabric-1.20.1.json");
    assert_eq!(child.inherits_from.as_deref(), Some("1.20.1"));
    let m = merge(parent.clone(), child.clone());
    assert_eq!(m.id, child.id);
    assert_eq!(
        m.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert!(m
        .downloads
        .as_ref()
        .unwrap()
        .client
        .url
        .ends_with("client.jar"));
    assert_eq!(
        m.libraries.len(),
        parent.libraries.len() + child.libraries.len()
    );
    assert_eq!(
        m.libraries[0].name, child.libraries[0].name,
        "child libraries come first"
    );
    let jvm = &m.arguments.as_ref().unwrap().jvm;
    assert!(jvm.len() > parent.arguments.as_ref().unwrap().jvm.len());
    assert!(m.inherits_from.is_none());
    // Forge/NeoForge ignoreLists name "${version_name}.jar": the client jar must be the profile's own.
    assert_eq!(m.jar_id(), m.id);
}

#[test]
fn merge_dedupes_same_artifact_keeping_child() {
    let mut parent = version("1.20.1.json");
    let mut child = version("fabric-1.20.1.json");
    let mut dup = parent.libraries[2].clone(); // com.mojang:logging
    dup.name = "com.mojang:logging:9.9.9".into();
    child.libraries.push(dup);
    parent.inherits_from = None;
    let m = merge(parent, child);
    let logging: Vec<_> = m
        .libraries
        .iter()
        .filter(|l| l.name.starts_with("com.mojang:logging:"))
        .collect();
    assert_eq!(logging.len(), 1);
    assert_eq!(logging[0].name, "com.mojang:logging:9.9.9");
}

#[test]
fn compare_handles_year_based_versions() {
    use std::cmp::Ordering::*;
    assert_eq!(compare_versions("26.3", "1.21.4"), Greater);
    assert_eq!(compare_versions("1.20.5", "1.20.10"), Less);
    assert_eq!(compare_versions("1.20", "1.20.0"), Equal);
    assert_eq!(compare_versions("1.21.1", "1.21.1"), Equal);
}

#[test]
fn environment_current_is_sane() {
    let e = Environment::current();
    assert!(["x86_64", "arm64", "x86"].contains(&e.arch.as_str()) || !e.arch.is_empty());
}
