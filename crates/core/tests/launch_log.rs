#![cfg(unix)]
//! Alone in its own test binary: tracing caches "nobody listens" per callsite
//! process-wide, so other tests launching without a subscriber would hide the logs.

mod common;

use cairn_core::launch::launch_instance;
use common::{collector, noop, setup};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);
impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn launcher_log_never_contains_token() {
    let (_d, core, id, acc) = setup(None).await;
    let buf = Buf::default();
    let b2 = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || b2.clone())
        .with_ansi(false)
        .finish();
    let _g = tracing::subscriber::set_default(sub);
    // Other tests may have hit these log callsites with no subscriber, which
    // tracing caches globally as "never log"; re-evaluate now.
    tracing::callsite::rebuild_interest_cache();
    // Offline sessions use "0" as the token; give the session a recognisable one.
    let (_l, log) = collector();
    launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap();
    let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    assert!(text.contains("-Xmx1024M"), "launch command logged: {text}");
    assert!(
        text.contains("--accessToken ***") || text.contains("\"--accessToken\", \"***\""),
        "token redacted: {text}"
    );
}
