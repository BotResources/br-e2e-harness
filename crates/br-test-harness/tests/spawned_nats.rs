#![cfg(all(feature = "spawned-nats", feature = "nats"))]

use std::time::Duration;

use br_test_harness::SpawnedNats;
use br_test_harness::nats::connect;
use futures_util::StreamExt as _;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn spawns_a_server_a_client_can_round_trip_against_its_reported_port() {
    let server = SpawnedNats::start().await;
    let url = server.url();

    assert!(
        url.ends_with(&format!(":{}", server.port())),
        "the reported url must carry the bound port: {url}"
    );

    let client = connect(&url)
        .await
        .expect("a client must connect to the spawned server's reported port");

    let mut sub = client
        .subscribe("harness.selftest")
        .await
        .expect("subscribe must succeed");
    client
        .publish("harness.selftest", "ping".into())
        .await
        .expect("publish must succeed");
    client.flush().await.expect("flush must succeed");

    let msg = tokio::time::timeout(Duration::from_secs(5), sub.next())
        .await
        .expect("the published message must arrive before the deadline")
        .expect("the subscription must yield the message");
    assert_eq!(msg.payload.as_ref(), b"ping");

    drop(client);
    server.shutdown().await;

    let addr = format!("127.0.0.1:{}", server_port_from(&url));
    let after = tokio::time::timeout(Duration::from_secs(5), wait_until_refused(&addr)).await;
    assert!(
        after.is_ok(),
        "the spawned server must stop accepting once shut down — a leaked process keeps {addr} open"
    );
}

fn non_loopback_local_ip() -> Option<std::net::IpAddr> {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect("192.0.2.1:9").ok()?;
    let ip = probe.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_spawned_server_listens_on_loopback_only() {
    let Some(lan_ip) = non_loopback_local_ip() else {
        eprintln!("no non-loopback interface on this host: the exposure cannot exist here");
        return;
    };
    let server = SpawnedNats::start().await;

    let reachable_on_lan = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect((lan_ip, server.port())),
    )
    .await;
    let loopback = tokio::net::TcpStream::connect(("127.0.0.1", server.port())).await;
    server.shutdown().await;

    assert!(
        loopback.is_ok(),
        "the server must accept on 127.0.0.1, the address url() reports"
    );
    assert!(
        !matches!(reachable_on_lan, Ok(Ok(_))),
        "the unauthenticated JetStream server must not accept on the non-loopback address {lan_ip}"
    );
}

fn server_port_from(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .expect("url must end with a port")
}

async fn wait_until_refused(addr: &str) {
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

const ORPHAN_CHILD: &str = "BR_HARNESS_SPAWNED_NATS_ORPHAN_CHILD";

async fn run_as_orphan_child_if_requested() -> bool {
    if std::env::var_os(ORPHAN_CHILD).is_none() {
        return false;
    }
    let server = SpawnedNats::start().await;
    println!("PORT={}", server.port());
    tokio::time::sleep(Duration::from_secs(600)).await;
    true
}

fn orphan_child_command(test: &str, scratch: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().expect("current exe"));
    command
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            test,
        ])
        .env(ORPHAN_CHILD, "1")
        .env("TMPDIR", scratch)
        .stdout(std::process::Stdio::piped());
    command
}

async fn assert_server_stopped_and_store_removed(addr: &str, scratch: &std::path::Path) {
    let stopped = tokio::time::timeout(Duration::from_secs(15), wait_until_refused(addr)).await;
    assert!(
        stopped.is_ok(),
        "a nats-server whose test process was killed must not keep {addr} open"
    );
    let emptied = tokio::time::timeout(Duration::from_secs(15), async {
        while std::fs::read_dir(scratch)
            .expect("read the scratch dir")
            .next()
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        emptied.is_ok(),
        "the killed process's store directory must be removed from the temp folder"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_killed_test_process_does_not_leave_its_nats_server_running() {
    if run_as_orphan_child_if_requested().await {
        return;
    }

    let scratch = tempfile::tempdir().expect("scratch temp dir");
    let mut child = orphan_child_command(
        "a_killed_test_process_does_not_leave_its_nats_server_running",
        scratch.path(),
    )
    .spawn()
    .expect("spawn the child test process");
    let port = read_port(child.stdout.take().expect("child stdout")).await;
    let addr = format!("127.0.0.1:{port}");
    assert!(
        tokio::net::TcpStream::connect(&addr).await.is_ok(),
        "the child's server must be up before the child is killed"
    );

    child.kill().expect("SIGKILL the child test process");
    child.wait().expect("reap the child test process");

    assert_server_stopped_and_store_removed(&addr, scratch.path()).await;
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_process_group_kill_does_not_leave_its_nats_server_or_store() {
    use std::os::unix::process::CommandExt as _;

    if run_as_orphan_child_if_requested().await {
        return;
    }

    let scratch = tempfile::tempdir().expect("scratch temp dir");
    let mut child = orphan_child_command(
        "a_process_group_kill_does_not_leave_its_nats_server_or_store",
        scratch.path(),
    )
    .process_group(0)
    .spawn()
    .expect("spawn the child test process in its own process group");
    let port = read_port(child.stdout.take().expect("child stdout")).await;
    let addr = format!("127.0.0.1:{port}");
    assert!(
        tokio::net::TcpStream::connect(&addr).await.is_ok(),
        "the child's server must be up before its group is killed"
    );

    let group = format!("-{}", child.id());
    let killed = std::process::Command::new("kill")
        .args(["-9", "--", &group])
        .status()
        .expect("run kill");
    assert!(killed.success(), "SIGKILL the child's whole process group");
    child.wait().expect("reap the child test process");

    assert_server_stopped_and_store_removed(&addr, scratch.path()).await;
}

async fn read_port(stdout: std::process::ChildStdout) -> u16 {
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead as _;
        std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .find_map(|line| {
                line.split_once("PORT=")
                    .and_then(|(_, port)| port.parse().ok())
            })
            .expect("the child test process must print its server port")
    })
    .await
    .expect("read the child's port")
}
