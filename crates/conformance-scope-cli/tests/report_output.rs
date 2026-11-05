#[path = "common/attached.rs"]
mod attached;
#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "common/scratch.rs"]
mod scratch;

use attached::AttachedService;
use cli::cli;
use fixture::{SCOPES, SERVICE_KEY};
use scratch::scratch;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_run_writes_a_junit_file_with_the_counts_and_the_failure_element() {
    let service = AttachedService::boot().await;
    let (nats_url, readyz_url) = (service.nats_url.clone(), service.readyz_url.clone());
    let dir = scratch("junit-output");
    let output = dir.join("conformance-scope.xml");
    let output_arg = output.to_string_lossy().to_string();

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                "notifier:read",
                "--scenarios",
                "declaration-content",
                "--format",
                "junit",
                "--output",
                &output_arg,
            ])
            .assert()
            .code(1)
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");
    service.shutdown().await;

    assert!(
        stdout.is_empty(),
        "with --output the report goes to the file only: {}",
        String::from_utf8_lossy(&stdout)
    );
    let xml = std::fs::read_to_string(&output).expect("the junit file was written");
    std::fs::remove_dir_all(&dir).ok();
    assert!(xml.starts_with("<?xml"), "{xml}");
    assert!(
        xml.contains("<testsuites tests=\"1\" failures=\"1\" errors=\"0\" skipped=\"0\">"),
        "{xml}"
    );
    assert!(
        xml.contains("<testcase classname=\"notifier\" name=\"declaration-content\">"),
        "{xml}"
    );
    assert!(xml.contains("<failure message=\""), "{xml}");
    assert!(xml.contains("scope set mismatch"), "{xml}");
}

fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

async fn serve_readyz_body(body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the /readyz stand-in");
    let url = format!(
        "http://{}/readyz",
        listener.local_addr().expect("stand-in address")
    );
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut connection, _)) = listener.accept().await else {
                return;
            };
            let mut request = [0u8; 1024];
            if connection.read(&mut request).await.is_err() {
                continue;
            }
            let answer = format!(
                "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            connection.write_all(answer.as_bytes()).await.ok();
            connection.flush().await.ok();
        }
    });
    (url, task)
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn junit_output_stays_well_formed_when_the_readyz_body_holds_ansi_escapes() {
    let service = AttachedService::boot().await;
    let nats_url = service.nats_url.clone();
    let (readyz_url, stand_in) = serve_readyz_body("\u{1b}[31mnot ready\u{1b}[0m \u{FFFE}").await;
    let dir = scratch("junit-ansi");
    let output = dir.join("conformance-scope.xml");
    let output_arg = output.to_string_lossy().to_string();

    tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
                "--reject",
                "--scenarios",
                "s4",
                "--timeout",
                "2s",
                "--format",
                "junit",
                "--output",
                &output_arg,
            ])
            .assert()
            .code(1);
    })
    .await
    .expect("cli task");
    stand_in.abort();
    service.shutdown().await;

    let xml = std::fs::read_to_string(&output).expect("the junit file was written");
    std::fs::remove_dir_all(&dir).ok();
    assert!(xml.chars().all(is_xml_char), "{xml:?}");
    assert!(xml.contains("<failure message=\""), "{xml:?}");
    assert!(xml.contains("[31mnot ready"), "{xml:?}");
}
