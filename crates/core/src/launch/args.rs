//! Turn a version JSON + instance + account into a `java ...` command line.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::auth::Session;
use crate::instances::Instance;
use crate::java::JavaInstall;
use crate::versions::install::{applicable_libraries, client_jar_path, library_path};
use crate::versions::maven::artifact_key;
use crate::versions::rules::{rules_allow, Environment};
use crate::versions::version_json::{Arg, VersionJson};
use crate::{Paths, Result};

pub struct LaunchContext<'a> {
    pub version: &'a VersionJson,
    pub instance: &'a Instance,
    pub session: &'a Session,
    pub java: &'a JavaInstall,
    pub paths: &'a Paths,
    pub env: &'a Environment,
    pub natives_dir: PathBuf,
    pub window: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Values that must never appear in logs.
    secrets: Vec<String>,
}

impl LaunchCommand {
    /// The arguments with the access token replaced by `***`, for logs.
    pub fn redacted(&self) -> Vec<String> {
        let mut prev_was_token_flag = false;
        self.args
            .iter()
            .map(|a| {
                let out = if prev_was_token_flag || self.secrets.iter().any(|s| s == a) {
                    "***".to_string()
                } else {
                    let mut a = a.clone();
                    // Tokens embedded in longer arguments (e.g. legacy "token:<t>:<uuid>").
                    for s in self.secrets.iter().filter(|s| s.len() >= 8) {
                        a = a.replace(s.as_str(), "***");
                    }
                    a
                };
                prev_was_token_flag = a == "--accessToken" || a == "--session";
                out
            })
            .collect()
    }
}

pub fn game_dir(paths: &Paths, instance: &Instance) -> PathBuf {
    paths.instance_dir(&instance.id).join("minecraft")
}

/// Where very old versions expect their assets (`${game_assets}`).
pub fn game_assets_dir(v: &VersionJson, paths: &Paths, game_dir: &Path) -> PathBuf {
    match v.assets.as_deref() {
        Some("legacy") => paths.assets_dir().join("virtual").join("legacy"),
        Some("pre-1.6") => game_dir.join("resources"),
        _ => paths.assets_dir(),
    }
}

/// Library jars on the classpath (no natives-only entries), each artifact
/// once, then the client jar.
pub fn classpath(v: &VersionJson, env: &Environment, paths: &Paths) -> Result<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for lib in applicable_libraries(v, env) {
        if !seen.insert(artifact_key(&lib.name)) {
            continue;
        }
        if let Some(p) = library_path(paths, lib)? {
            out.push(p);
        }
    }
    out.push(client_jar_path(paths, v));
    Ok(out)
}

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn expand(args: &[Arg], env: &Environment, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(substitute(s, vars)),
            Arg::Conditional { rules, value } => {
                if rules_allow(rules, env) {
                    out.extend(value.values().into_iter().map(|s| substitute(s, vars)));
                }
            }
        }
    }
    out
}

fn program(java: &JavaInstall) -> PathBuf {
    // On Windows, javaw starts the game without a console window.
    if cfg!(windows) {
        let w = java.path.with_file_name("javaw.exe");
        if w.is_file() {
            return w;
        }
    }
    java.path.clone()
}

pub fn build_command(ctx: &LaunchContext) -> Result<LaunchCommand> {
    let v = ctx.version;
    let gdir = game_dir(ctx.paths, ctx.instance);
    let sep = if ctx.env.os == crate::versions::rules::OsName::Windows {
        ";"
    } else {
        ":"
    };
    let cp: Vec<String> = classpath(v, ctx.env, ctx.paths)?
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let mut env = ctx.env.clone();
    env.features.insert("is_demo_user".into(), false);
    env.features
        .insert("has_custom_resolution".into(), ctx.window.is_some());

    let s = |p: &Path| p.to_string_lossy().into_owned();
    let (w, h) = ctx
        .window
        .map(|(w, h)| (w.to_string(), h.to_string()))
        .unwrap_or_default();
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", ctx.session.username.clone());
    vars.insert("version_name", v.id.clone());
    vars.insert("game_directory", s(&gdir));
    vars.insert("assets_root", s(&ctx.paths.assets_dir()));
    vars.insert(
        "assets_index_name",
        v.asset_index
            .as_ref()
            .map(|a| a.id.clone())
            .or_else(|| v.assets.clone())
            .unwrap_or_default(),
    );
    vars.insert("auth_uuid", ctx.session.uuid.clone());
    vars.insert("auth_access_token", ctx.session.access_token.clone());
    vars.insert(
        "auth_session",
        format!("token:{}:{}", ctx.session.access_token, ctx.session.uuid),
    );
    vars.insert(
        "clientid",
        crate::auth::msa_client_id().unwrap_or("").to_string(),
    );
    vars.insert("auth_xuid", ctx.session.xuid.clone().unwrap_or_default());
    vars.insert("user_type", ctx.session.user_type.clone());
    vars.insert(
        "version_type",
        v.kind.clone().unwrap_or_else(|| "release".into()),
    );
    vars.insert("natives_directory", s(&ctx.natives_dir));
    vars.insert("launcher_name", "cairn".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", cp.join(sep));
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", s(&ctx.paths.libraries_dir()));
    vars.insert("game_assets", s(&game_assets_dir(v, ctx.paths, &gdir)));
    vars.insert("user_properties", "{}".into());
    vars.insert("resolution_width", w.clone());
    vars.insert("resolution_height", h.clone());

    let mut args = vec![format!("-Xmx{}M", ctx.instance.memory_mb)];
    match v.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(a) => args.extend(expand(&a.jvm, &env, &vars)),
        None => {
            args.push(substitute(
                "-Djava.library.path=${natives_directory}",
                &vars,
            ));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    if let Some(c) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let file = ctx.paths.assets_dir().join("log_configs").join(&c.file.id);
        args.push(c.argument.replace("${path}", &s(&file)));
    }
    args.extend(ctx.instance.jvm_args.iter().cloned());
    args.push(v.main_class.clone());
    match (&v.arguments, &v.minecraft_arguments) {
        (_, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(|t| substitute(t, &vars)));
            if ctx.window.is_some() {
                args.extend(["--width".into(), w, "--height".into(), h]);
            }
        }
        (Some(a), None) => args.extend(expand(&a.game, &env, &vars)),
        (None, None) => {}
    }

    Ok(LaunchCommand {
        program: program(ctx.java),
        args,
        cwd: gdir,
        secrets: vec![ctx.session.access_token.clone()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Session;
    use crate::instances::{Instance, JavaSetting};
    use crate::java::JavaInstall;
    use crate::versions::rules::OsName;
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;

    fn version(name: &str) -> VersionJson {
        serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/tests/fixtures/versions/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap()
    }
    fn instance() -> Instance {
        Instance {
            schema: 1,
            id: "survival".into(),
            name: "Survival".into(),
            minecraft: "1.20.1".into(),
            loader: None,
            java: JavaSetting::Auto,
            memory_mb: 6144,
            jvm_args: vec!["-XX:+UseG1GC".into()],
            created_at: chrono::Utc::now(),
            last_played_at: None,
            mods: BTreeMap::new(),
            launch_version: None,
        }
    }
    fn session() -> Session {
        Session {
            username: "PlayerOne".into(),
            uuid: "abc123".into(),
            access_token: "secret-token".into(),
            user_type: "msa".into(),
            xuid: Some("42".into()),
        }
    }
    fn java() -> JavaInstall {
        JavaInstall {
            path: PathBuf::from("/java/bin/java"),
            major: 17,
            version: "17.0.9".into(),
            arch: "x86_64".into(),
            managed: true,
        }
    }
    fn linux() -> Environment {
        Environment {
            os: OsName::Linux,
            arch: "x86_64".into(),
            os_version: String::new(),
            features: HashMap::new(),
        }
    }
    fn build(v: &VersionJson, root: &str, window: Option<(u32, u32)>) -> LaunchCommand {
        let paths = Paths::new(PathBuf::from(root));
        let (i, s, j, e) = (instance(), session(), java(), linux());
        let ctx = LaunchContext {
            version: v,
            instance: &i,
            session: &s,
            java: &j,
            paths: &paths,
            env: &e,
            natives_dir: PathBuf::from(root).join("natives"),
            window,
        };
        build_command(&ctx).unwrap()
    }
    fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
        &args[args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} missing in {args:?}"))
            + 1]
    }

    #[test]
    fn modern_command_has_expected_args() {
        let cmd = build(&version("1.20.1.json"), "/r", None);
        let a = &cmd.args;
        assert_eq!(cmd.program, PathBuf::from("/java/bin/java"));
        assert_eq!(a[0], "-Xmx6144M");
        assert!(a.contains(&"net.minecraft.client.main.Main".to_string()));
        assert_eq!(after(a, "--username"), "PlayerOne");
        assert_eq!(after(a, "--uuid"), "abc123");
        assert_eq!(after(a, "--accessToken"), "secret-token");
        assert_eq!(
            PathBuf::from(after(a, "--gameDir")),
            PathBuf::from("/r/instances/survival/minecraft")
        );
        assert_eq!(after(a, "--assetIndex"), "5");
        assert!(!a.contains(&"--demo".to_string()));
        assert!(!a.iter().any(|x| x.contains("quickPlay")));
        assert!(
            !a.iter().any(|x| x.contains("${")),
            "unresolved placeholder: {a:?}"
        );
        assert!(
            a.iter()
                .any(|x| x.starts_with("-Dlog4j.configurationFile=")
                    && x.ends_with("client-1.12.xml"))
        );
        assert!(
            !a.contains(&"-XstartOnFirstThread".to_string()),
            "osx-only arg on linux"
        );
        let main = a
            .iter()
            .position(|x| x == "net.minecraft.client.main.Main")
            .unwrap();
        assert!(
            a.iter().position(|x| x == "-XX:+UseG1GC").unwrap() < main,
            "instance jvm args go before the main class"
        );
        assert_eq!(
            PathBuf::from(&cmd.cwd),
            PathBuf::from("/r/instances/survival/minecraft")
        );
    }

    #[test]
    fn legacy_command_uses_minecraft_arguments() {
        let cmd = build(&version("1.8.9.json"), "/r", None);
        let a = &cmd.args;
        assert!(a.iter().any(|x| x.starts_with("-Djava.library.path=")));
        assert!(a.contains(&"-cp".to_string()));
        assert_eq!(after(a, "--userProperties"), "{}");
        assert_eq!(after(a, "--userType"), "msa");
        assert_eq!(after(a, "--version"), "1.8.9");
        assert!(!a.iter().any(|x| x.contains("${")));
    }

    #[test]
    fn custom_resolution_feature() {
        let cmd = build(&version("1.20.1.json"), "/r", Some((1280, 720)));
        assert_eq!(after(&cmd.args, "--width"), "1280");
        assert_eq!(after(&cmd.args, "--height"), "720");
    }

    #[test]
    fn build_command_handles_spaces_and_non_ascii() {
        let root = "/tmp/张 三/Cairn";
        let cmd = build(&version("1.20.1.json"), root, None);
        assert_eq!(
            PathBuf::from(after(&cmd.args, "--gameDir")),
            PathBuf::from(root).join("instances/survival/minecraft")
        );
        assert!(after(&cmd.args, "-cp").contains("张 三"));
    }

    #[test]
    fn redacted_hides_token() {
        let cmd = build(&version("1.20.1.json"), "/r", None);
        let r = cmd.redacted();
        assert!(!r.iter().any(|x| x.contains("secret-token")));
        assert!(r.contains(&"***".to_string()));
    }

    #[test]
    fn classpath_dedupes_and_ends_with_client_jar() {
        let mut v = version("1.20.1.json");
        let dup = v
            .libraries
            .iter()
            .find(|l| l.rules.is_empty())
            .unwrap()
            .clone();
        v.libraries.push(dup);
        let paths = Paths::new(PathBuf::from("/r"));
        let cp = classpath(&v, &linux(), &paths).unwrap();
        let unique: std::collections::HashSet<_> = cp.iter().collect();
        assert_eq!(unique.len(), cp.len());
        assert_eq!(
            cp.last().unwrap(),
            &PathBuf::from("/r/shared/versions/1.20.1/1.20.1.jar")
        );
        assert!(
            !cp.iter()
                .any(|p| p.to_string_lossy().contains("java-objc-bridge")),
            "osx-only library excluded"
        );
    }

    #[test]
    fn legacy_asset_layouts() {
        let paths = Paths::new(PathBuf::from("/r"));
        let g = PathBuf::from("/g");
        let mut v = version("1.8.9.json");
        assert_eq!(
            game_assets_dir(&v, &paths, &g),
            PathBuf::from("/r/shared/assets")
        );
        v.assets = Some("legacy".into());
        assert_eq!(
            game_assets_dir(&v, &paths, &g),
            PathBuf::from("/r/shared/assets/virtual/legacy")
        );
        v.assets = Some("pre-1.6".into());
        assert_eq!(
            game_assets_dir(&v, &paths, &g),
            PathBuf::from("/g/resources")
        );
    }
}
