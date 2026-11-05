use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cairn_core::download::Progress;
use cairn_core::instances::{LoaderKind, LoaderSpec};
use cairn_core::java::find_installations;
use cairn_core::loaders::forge::neoforge_mc;
use cairn_core::loaders::{install_loader, list_loader_versions};
use cairn_core::versions::install::load_installed;
use cairn_core::versions::maven::maven_path;
use cairn_core::{Core, Endpoints, Paths};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}
fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}

async fn listing_server() -> MockServer {
    let server = MockServer::start().await;
    let xml = "<metadata><versioning><versions><version>1.21.1-52.1.20</version><version>1.20.1-47.4.27</version><version>1.20.1-47.4.28</version></versions></versioning></metadata>";
    Mock::given(method("GET"))
        .and(path("/net/minecraftforge/forge/maven-metadata.xml"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/net/minecraftforge/forge/promotions_slim.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"promos": {"1.20.1-recommended": "47.4.27", "1.20.1-latest": "47.4.28"}}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/api/maven/versions/releases/net/neoforged/neoforge"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"versions": ["0.25w14craftmine.3-beta", "20.4.1-beta", "21.1.76", "21.1.77", "26.1.2.5", "26.3.0.101"]}))).mount(&server).await;
    server
}

fn core_at(server: &MockServer) -> (tempfile::TempDir, Core) {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().into()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    (dir, core)
}

#[test]
fn neoforge_version_to_mc() {
    assert_eq!(neoforge_mc("21.1.77").as_deref(), Some("1.21.1"));
    assert_eq!(neoforge_mc("21.0.3-beta").as_deref(), Some("1.21"));
    assert_eq!(neoforge_mc("20.4.237").as_deref(), Some("1.20.4"));
    assert_eq!(neoforge_mc("26.3.0.101").as_deref(), Some("26.3"));
    assert_eq!(neoforge_mc("26.1.2.5").as_deref(), Some("26.1.2"));
    assert_eq!(neoforge_mc("0.25w14craftmine.3-beta"), None);
}

#[tokio::test]
async fn lists_forge_versions_for_mc_and_marks_recommended() {
    let server = listing_server().await;
    let (_d, core) = core_at(&server);
    let list = list_loader_versions(&core, LoaderKind::Forge, "1.20.1")
        .await
        .unwrap();
    let got: Vec<_> = list
        .iter()
        .map(|v| (v.version.as_str(), v.stable))
        .collect();
    assert_eq!(got, [("47.4.28", false), ("47.4.27", true)]);
}

#[tokio::test]
async fn maps_neoforge_versions_to_mc() {
    let server = listing_server().await;
    let (_d, core) = core_at(&server);
    let v = |mc: &'static str| {
        let core = &core;
        async move {
            list_loader_versions(core, LoaderKind::NeoForge, mc)
                .await
                .unwrap()
                .into_iter()
                .map(|v| (v.version, v.stable))
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        v("1.21.1").await,
        [("21.1.77".to_string(), true), ("21.1.76".to_string(), true)]
    );
    assert_eq!(v("26.3").await, [("26.3.0.101".to_string(), true)]);
    assert_eq!(v("1.20.4").await, [("20.4.1-beta".to_string(), false)]);
}

// ---------- installer fixtures ----------

const MAIN_JAVA: &str = r#"
import java.nio.file.*;
public class Main {
  public static void main(String[] a) throws Exception {
    if (a.length > 0 && a[0].equals("--fail")) System.exit(3);
    String out = null;
    for (int i = 0; i < a.length; i++) if (a[i].equals("--out")) out = a[i + 1];
    Files.createDirectories(Paths.get(out).getParent());
    String text = String.join("\n", a);
    for (int i = 0; i < a.length; i++)
      if (a[i].equals("--patch")) text += "\n--patch-content\n" + new String(Files.readAllBytes(Paths.get(a[i + 1])), "UTF-8");
    Files.write(Paths.get(out), text.getBytes("UTF-8"));
  }
}
"#;

/// Compile a tiny processor jar. None (test skipped) when javac is missing.
fn processor_jar() -> Option<Vec<u8>> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Main.java"), MAIN_JAVA).unwrap();
    let ok = std::process::Command::new("javac")
        .arg("--release")
        .arg("8")
        .arg("-d")
        .arg(dir.path())
        .arg(dir.path().join("Main.java"))
        .status()
        .ok()?
        .success();
    if !ok {
        eprintln!("javac failed; skipping");
        return None;
    }
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("META-INF/MANIFEST.MF", o).unwrap();
        z.write_all(b"Manifest-Version: 1.0\r\nMain-Class: Main\r\n\r\n")
            .unwrap();
        z.start_file("Main.class", o).unwrap();
        z.write_all(&std::fs::read(dir.path().join("Main.class")).unwrap())
            .unwrap();
        z.finish().unwrap();
    }
    Some(buf)
}

fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        for (name, body) in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(body).unwrap();
        }
        z.finish().unwrap();
    }
    buf
}

/// A vanilla JSON with no downloads, asking for whatever Java this machine has.
fn place_vanilla(core: &Core, mc: &str) {
    let major = find_installations(&core.paths)
        .first()
        .map(|j| j.major)
        .unwrap_or(21);
    let v = json!({"id": mc, "mainClass": "net.minecraft.client.main.Main", "libraries": [], "javaVersion": {"majorVersion": major}, "type": "release"});
    let d = core.paths.versions_dir().join(mc);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join(format!("{mc}.json")),
        serde_json::to_vec(&v).unwrap(),
    )
    .unwrap();
}

async fn serve_lib(server: &MockServer, coord: &str, body: Vec<u8>) -> Value {
    let p = maven_path(coord).unwrap();
    Mock::given(method("GET"))
        .and(path(format!("/maven/{p}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .mount(server)
        .await;
    json!({"name": coord, "downloads": {"artifact": {"path": p, "url": format!("{}/maven/{p}", server.uri()), "sha1": sha1_hex(&body), "size": body.len()}}})
}

const MV: &str = "1.20.1-47.4.28";

async fn serve_installer(server: &MockServer, profile: Value, version: Value) {
    let installer = zip_of(&[
        (
            "install_profile.json",
            serde_json::to_vec(&profile).unwrap(),
        ),
        ("version.json", serde_json::to_vec(&version).unwrap()),
        ("data/client.lzma", b"binpatch".to_vec()),
        (
            &format!("maven/net/minecraftforge/forge/{MV}/forge-{MV}-universal.jar"),
            b"universal".to_vec(),
        ),
    ]);
    Mock::given(method("GET"))
        .and(path(format!(
            "/net/minecraftforge/forge/{MV}/forge-{MV}-installer.jar"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(installer))
        .mount(server)
        .await;
}

fn forge_version_json() -> Value {
    json!({
        "id": "1.20.1-forge-47.4.28", "inheritsFrom": "1.20.1", "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher", "type": "release",
        "libraries": [{"name": format!("net.minecraftforge:forge:{MV}:client"), "downloads": {"artifact": {"path": format!("net/minecraftforge/forge/{MV}/forge-{MV}-client.jar"), "url": "", "sha1": "x", "size": 1}}}],
        "arguments": {"game": ["--launchTarget", "forgeclient"], "jvm": []}
    })
}

fn lib_file(core: &Core, coord: &str) -> PathBuf {
    core.paths.libraries_dir().join(maven_path(coord).unwrap())
}

#[tokio::test]
async fn runs_client_processors_with_substituted_args() {
    let Some(jar) = processor_jar() else { return };
    let server = MockServer::start().await;
    let proc_lib = serve_lib(&server, "com.example:proc:1.0", jar).await;
    let dep_lib = serve_lib(&server, "com.example:dep:1.0", b"dep".to_vec()).await;
    let profile = json!({
        "spec": 1, "minecraft": "1.20.1", "json": "/version.json",
        "data": {
            "OUT": {"client": "[com.example:out:1.0@txt]", "server": "[com.example:out-server:1.0@txt]"},
            "LIT": {"client": "'hello'", "server": "'nope'"},
            "PATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"}
        },
        "processors": [
            {"sides": ["server"], "jar": "com.example:proc:1.0", "classpath": [], "args": ["--fail"]},
            {"jar": "com.example:proc:1.0", "classpath": ["com.example:dep:1.0"],
             "args": ["--out", "{OUT}", "--mc", "{MINECRAFT_JAR}", "--lib", "[com.example:dep:1.0]", "--lit", "{LIT}", "--patch", "{PATCH}", "--side", "{SIDE}"]}
        ],
        "libraries": [proc_lib, dep_lib]
    });
    serve_installer(&server, profile, forge_version_json()).await;
    let (_d, core) = core_at(&server);
    place_vanilla(&core, "1.20.1");

    let spec = LoaderSpec {
        kind: LoaderKind::Forge,
        version: "47.4.28".into(),
    };
    let id = install_loader(&core, "1.20.1", &spec, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(id, "1.20.1-forge-47.4.28");
    assert!(load_installed(&core.paths, &id).unwrap().is_some());
    assert!(
        lib_file(&core, &format!("net.minecraftforge:forge:{MV}:universal")).exists(),
        "maven/ extracted from installer"
    );

    let out = std::fs::read_to_string(lib_file(&core, "com.example:out:1.0@txt")).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    let arg = |k: &str| lines[lines.iter().position(|l| *l == k).unwrap() + 1];
    assert_eq!(
        Path::new(arg("--mc")),
        core.paths.versions_dir().join("1.20.1/1.20.1.jar")
    );
    assert_eq!(
        Path::new(arg("--lib")),
        lib_file(&core, "com.example:dep:1.0")
    );
    assert_eq!(arg("--lit"), "hello");
    assert_eq!(arg("--side"), "client");
    assert!(arg("--patch")
        .replace('\\', "/")
        .ends_with("data/client.lzma"));
    assert_eq!(arg("--patch-content"), "binpatch");
}

#[tokio::test]
async fn skips_processor_when_outputs_match() {
    let Some(jar) = processor_jar() else { return };
    let server = MockServer::start().await;
    let proc_lib = serve_lib(&server, "com.example:proc:1.0", jar).await;
    let existing = b"already patched".to_vec();
    let profile = json!({
        "spec": 1, "minecraft": "1.20.1", "json": "/version.json",
        "data": {"OUT": {"client": "[com.example:out:1.0@txt]", "server": "x"}, "OUT_SHA": {"client": format!("'{}'", sha1_hex(&existing)), "server": "x"}},
        "processors": [{"jar": "com.example:proc:1.0", "classpath": [], "args": ["--fail"], "outputs": {"{OUT}": "{OUT_SHA}"}}],
        "libraries": [proc_lib]
    });
    serve_installer(&server, profile, forge_version_json()).await;
    let (_d, core) = core_at(&server);
    place_vanilla(&core, "1.20.1");
    let out = lib_file(&core, "com.example:out:1.0@txt");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, &existing).unwrap();
    let spec = LoaderSpec {
        kind: LoaderKind::Forge,
        version: "47.4.28".into(),
    };
    install_loader(&core, "1.20.1", &spec, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), existing);
}

#[tokio::test]
async fn failing_processor_is_reported() {
    let Some(jar) = processor_jar() else { return };
    let server = MockServer::start().await;
    let proc_lib = serve_lib(&server, "com.example:proc:1.0", jar).await;
    let profile = json!({
        "spec": 1, "minecraft": "1.20.1", "json": "/version.json", "data": {},
        "processors": [{"jar": "com.example:proc:1.0", "classpath": [], "args": ["--fail"]}],
        "libraries": [proc_lib]
    });
    serve_installer(&server, profile, forge_version_json()).await;
    let (_d, core) = core_at(&server);
    place_vanilla(&core, "1.20.1");
    let spec = LoaderSpec {
        kind: LoaderKind::Forge,
        version: "47.4.28".into(),
    };
    let err = install_loader(&core, "1.20.1", &spec, noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.loader.processor_failed");
}

#[tokio::test]
async fn legacy_installer_format_extracts_universal_jar() {
    let server = MockServer::start().await;
    let lmv = "1.12.2-14.23.0.2486";
    let profile = json!({
        "install": {"profileName": "Forge", "target": "1.12.2-forge1.12.2-14.23.0.2486", "path": format!("net.minecraftforge:forge:{lmv}"), "version": "forge 14.23.0.2486", "filePath": format!("forge-{lmv}-universal.jar"), "minecraft": "1.12.2"},
        "versionInfo": {"id": "1.12.2-forge1.12.2-14.23.0.2486", "inheritsFrom": "1.12.2", "mainClass": "net.minecraft.launchwrapper.Launch", "minecraftArguments": "--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker", "libraries": [{"name": format!("net.minecraftforge:forge:{lmv}")}]}
    });
    let installer = zip_of(&[
        (
            "install_profile.json",
            serde_json::to_vec(&profile).unwrap(),
        ),
        (
            &format!("forge-{lmv}-universal.jar"),
            b"legacy universal".to_vec(),
        ),
    ]);
    Mock::given(method("GET"))
        .and(path(format!(
            "/net/minecraftforge/forge/{lmv}/forge-{lmv}-installer.jar"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(installer))
        .mount(&server)
        .await;
    let (_d, core) = core_at(&server);
    place_vanilla(&core, "1.12.2");
    let spec = LoaderSpec {
        kind: LoaderKind::Forge,
        version: "14.23.0.2486".into(),
    };
    let id = install_loader(&core, "1.12.2", &spec, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(id, "1.12.2-forge1.12.2-14.23.0.2486");
    assert_eq!(
        std::fs::read(lib_file(&core, &format!("net.minecraftforge:forge:{lmv}"))).unwrap(),
        b"legacy universal"
    );
    let v = load_installed(&core.paths, &id).unwrap().unwrap();
    assert_eq!(v.main_class, "net.minecraft.launchwrapper.Launch");
}
