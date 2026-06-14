use std::time::Duration;

use br_test_harness::{BootOutcome, SpawnedProcess};
use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpListener;

fn unbound_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to read a free port");
    let port = listener.local_addr().expect("read assigned port").port();
    drop(listener);
    format!("http://127.0.0.1:{port}/readyz")
}

async fn serve_one_ok() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind responder");
    let port = listener.local_addr().expect("read responder port").port();
    let handle = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await;
            let _ = stream.flush().await;
        }
    });
    (format!("http://127.0.0.1:{port}/readyz"), handle)
}

#[tokio::test]
async fn exit_before_ready_is_classified_with_captured_output() {
    let mut proc = SpawnedProcess::spawn(
        "/bin/sh",
        &["-c", "echo missing-stream CHARTER >&2; exit 3"],
        &[],
    );

    let outcome = proc
        .await_boot(&unbound_url(), Duration::from_secs(5))
        .await;

    match outcome {
        BootOutcome::Exited(status) => assert_eq!(status.code(), Some(3)),
        other => panic!("a process that exits before readiness must classify as Exited: {other:?}"),
    }
    assert!(
        proc.logs().contains("missing-stream CHARTER"),
        "the captured output must carry what the binary named before refusing to boot: {}",
        proc.logs()
    );
}

#[tokio::test]
async fn never_ready_process_times_out() {
    let mut proc = SpawnedProcess::spawn("/bin/sh", &["-c", "sleep 30"], &[]);

    let outcome = proc
        .await_boot(&unbound_url(), Duration::from_millis(400))
        .await;

    assert!(
        matches!(outcome, BootOutcome::TimedOut),
        "a running process that never serves /readyz must classify as TimedOut: {outcome:?}"
    );
    proc.shutdown().await;
}

#[tokio::test]
async fn readyz_returning_200_is_ready() {
    let (url, responder) = serve_one_ok().await;
    let mut proc = SpawnedProcess::spawn("/bin/sh", &["-c", "sleep 30"], &[]);

    let outcome = proc.await_boot(&url, Duration::from_secs(5)).await;

    assert!(
        outcome.is_ready(),
        "a 200 on /readyz must classify as Ready: {outcome:?}"
    );
    assert!(outcome.exit_status().is_none());
    proc.shutdown().await;
    responder.abort();
}
