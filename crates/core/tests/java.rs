// ensure_java = select an installed runtime, else download_java.
use std::io::Write;
use std::sync::Arc;

use cairn_core::download::Progress;
use cairn_core::java::adoptium::download_java;
use cairn_core::{Core, Endpoints, Paths};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fake_jre_tar_gz() -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_bytes);
        let add = |b: &mut tar::Builder<&mut Vec<u8>>, name: &str, body: &[u8], mode: u32| {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(mode);
            h.set_cksum();
            b.append_data(&mut h, name, body).unwrap();
        };
        add(
            &mut b,
            "jdk-17.0.9+9-jre/release",
            b"JAVA_VERSION=\"17.0.9\"\nOS_ARCH=\"x86_64\"\n",
            0o644,
        );
        let exe = if cfg!(windows) {
            "jdk-17.0.9+9-jre/bin/java.exe"
        } else {
            "jdk-17.0.9+9-jre/bin/java"
        };
        add(&mut b, exe, b"#!/bin/sh\n", 0o755);
        b.finish().unwrap();
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar_bytes).unwrap();
    gz.finish().unwrap()
}

async fn serve(pkg: Vec<u8>, checksum: String) -> MockServer {
    let server = MockServer::start().await;
    let body = json!([{
        "binary": {"package": {"link": format!("{}/pkg/OpenJDK17U-jre.tar.gz", server.uri()), "checksum": checksum, "name": "OpenJDK17U-jre.tar.gz", "size": pkg.len()}},
        "version": {"semver": "17.0.9+9", "openjdk_version": "17.0.9+9"}
    }]);
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/17/hotspot"))
        .and(query_param("image_type", "jre"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pkg/OpenJDK17U-jre.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
        .mount(&server)
        .await;
    server
}

fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

#[tokio::test]
async fn ensure_java_downloads_and_extracts() {
    let pkg = fake_jre_tar_gz();
    let sum = hex::encode(Sha256::digest(&pkg));
    let server = serve(pkg, sum).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().to_path_buf()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    // Major 17 might exist on the machine; ask for a major nothing has installed by mapping 17 → our mock.
    let j = download_java(&core, 17, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert!(j.path.exists(), "{:?}", j.path);
    assert!(j.managed);
    assert_eq!(j.major, 17);
    assert!(j.path.starts_with(core.paths.java_dir().join("temurin-17")));
}

#[tokio::test]
async fn ensure_java_rejects_bad_checksum() {
    let pkg = fake_jre_tar_gz();
    let server = serve(pkg, "00".repeat(32)).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().to_path_buf()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    let err = download_java(&core, 17, noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.java.checksum");
    assert!(!core.paths.java_dir().join("temurin-17").exists());
}
