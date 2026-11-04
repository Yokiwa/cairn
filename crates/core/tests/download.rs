use std::sync::{Arc, Mutex};
use std::time::Duration;

use cairn_core::download::{sha1_file, DownloadItem, Downloader, Progress, ProgressFn};
use cairn_core::http::HttpClient;
use cairn_core::tasks::{TaskEvent, TaskManager};
use cairn_core::AppError;
use sha1::{Digest, Sha1};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}

fn dl() -> Downloader {
    Downloader::new(
        HttpClient::new()
            .unwrap()
            .with_retry_base(Duration::from_millis(1)),
        4,
    )
    .with_retry_base(Duration::from_millis(1))
}

fn noop() -> ProgressFn {
    Arc::new(|_: &Progress| {})
}

const BODY: &[u8] = b"hello cairn downloader";

#[tokio::test]
async fn downloads_and_verifies() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sub/a.bin");
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest: dest.clone(),
        sha1: Some(sha1_hex(BODY)),
        size: Some(BODY.len() as u64),
    };
    dl().download_all("test", vec![item], noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), BODY);
    assert_eq!(sha1_file(&dest).unwrap(), sha1_hex(BODY));
}

#[tokio::test]
async fn skips_file_with_matching_sha1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .expect(0)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    std::fs::write(&dest, BODY).unwrap();
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest,
        sha1: Some(sha1_hex(BODY)),
        size: None,
    };
    dl().download_all("test", vec![item], noop(), CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn resumes_from_part_file() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .and(header("Range", "bytes=5-"))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(&BODY[5..]))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    std::fs::write(dir.path().join("a.bin.part"), &BODY[..5]).unwrap();
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest: dest.clone(),
        sha1: Some(sha1_hex(BODY)),
        size: None,
    };
    dl().download_all("test", vec![item], noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), BODY);
    assert!(!dir.path().join("a.bin.part").exists());
}

#[tokio::test]
async fn checksum_mismatch_retries_then_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wrong".to_vec()))
        .expect(4)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest: dest.clone(),
        sha1: Some(sha1_hex(BODY)),
        size: None,
    };
    let err = dl()
        .download_all("test", vec![item], noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.download.checksum");
    assert!(!dest.exists());
}

struct FailTwice(Arc<Mutex<u32>>);
impl Respond for FailTwice {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let mut n = self.0.lock().unwrap();
        *n += 1;
        if *n <= 2 {
            ResponseTemplate::new(500)
        } else {
            ResponseTemplate::new(200).set_body_bytes(BODY)
        }
    }
}

#[tokio::test]
async fn server_error_then_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(FailTwice(Arc::new(Mutex::new(0))))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest: dest.clone(),
        sha1: Some(sha1_hex(BODY)),
        size: None,
    };
    dl().download_all("test", vec![item], noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), BODY);
}

#[tokio::test]
async fn progress_reaches_totals() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let items: Vec<_> = (0..3)
        .map(|i| DownloadItem {
            url: format!("{}/f{i}", server.uri()),
            dest: dir.path().join(format!("f{i}")),
            sha1: None,
            size: Some(BODY.len() as u64),
        })
        .collect();
    let last = Arc::new(Mutex::new(None::<Progress>));
    let l2 = last.clone();
    let pf: ProgressFn = Arc::new(move |p: &Progress| *l2.lock().unwrap() = Some(p.clone()));
    dl().download_all("game", items, pf, CancellationToken::new())
        .await
        .unwrap();
    let p = last.lock().unwrap().clone().unwrap();
    assert_eq!((p.done_files, p.total_files), (3, 3));
    assert_eq!(p.stage, "game");
    assert_eq!(p.done_bytes, 3 * BODY.len() as u64);
}

#[tokio::test]
async fn cancel_stops_download() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(BODY)
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let item = DownloadItem {
        url: format!("{}/a", server.uri()),
        dest: dir.path().join("a"),
        sha1: None,
        size: None,
    };
    let token = CancellationToken::new();
    let t2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        t2.cancel();
    });
    let err = dl()
        .download_all("test", vec![item], noop(), token)
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.cancelled");
}

#[tokio::test]
async fn task_manager_emits_done_and_failed() {
    let tm = TaskManager::new();
    let mut rx = tm.subscribe();
    let ok = tm.spawn("ok".into(), |_p, _c| async { Ok(serde_json::json!(42)) });
    let bad = tm.spawn("bad".into(), |_p, _c| async {
        Err(AppError::new("error.x"))
    });
    let mut done = None;
    let mut failed = None;
    while done.is_none() || failed.is_none() {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            TaskEvent::Done { id, result } if id == ok => done = Some(result),
            TaskEvent::Failed { id, error } if id == bad => failed = Some(error),
            _ => {}
        }
    }
    assert_eq!(done.unwrap(), serde_json::json!(42));
    assert_eq!(failed.unwrap().key, "error.x");
    assert_eq!(tm.list().len(), 2);
}

#[tokio::test]
async fn task_manager_cancel_marks_cancelled() {
    let tm = TaskManager::new();
    let mut rx = tm.subscribe();
    let id = tm.spawn("slow".into(), |_p, c| async move {
        c.cancelled().await;
        Err(AppError::new("error.cancelled"))
    });
    tm.cancel(&id);
    loop {
        if let TaskEvent::Failed { id: got, error } =
            tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .unwrap()
                .unwrap()
        {
            assert_eq!(got, id);
            assert_eq!(error.key, "error.cancelled");
            break;
        }
    }
    let info = tm.list().into_iter().find(|t| t.id == id).unwrap();
    assert_eq!(info.state, cairn_core::tasks::TaskState::Cancelled);
}

#[tokio::test]
async fn get_json_cached_serves_stale_when_offline() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"v": 1})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let http = HttpClient::new()
        .unwrap()
        .with_retry_base(Duration::from_millis(1));
    let url = format!("{}/data.json", server.uri());
    let first: serde_json::Value = http
        .get_json_cached(&url, dir.path(), Duration::from_secs(0))
        .await
        .unwrap();
    assert_eq!(first["v"], 1);
    drop(server); // now "offline"
    let second: serde_json::Value = http
        .get_json_cached(&url, dir.path(), Duration::from_secs(0))
        .await
        .unwrap();
    assert_eq!(second["v"], 1);
}

#[tokio::test]
async fn get_json_reports_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let http = HttpClient::new()
        .unwrap()
        .with_retry_base(Duration::from_millis(1));
    let err = http
        .get_json::<serde_json::Value>(&format!("{}/x", server.uri()))
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.http");
    assert_eq!(err.params.get("status").map(String::as_str), Some("404"));
}
