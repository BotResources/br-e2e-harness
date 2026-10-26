use std::time::Duration;

use br_test_harness::{SpawnedProcess, run_once};

const ORPHAN_CHILD: &str = "BR_HARNESS_SPAWNED_PROCESS_ORPHAN_CHILD";
const SPAWN_TEST: &str = "a_killed_test_process_does_not_leave_its_spawned_process_running";
const RUN_ONCE_TEST: &str = "a_killed_test_process_does_not_leave_its_run_once_process_running";

#[tokio::test]
async fn a_killed_test_process_does_not_leave_its_spawned_process_running() {
    if std::env::var_os(ORPHAN_CHILD).is_some() {
        let proc = SpawnedProcess::spawn("sh", &["-c", "echo PID=$$; exec sleep 600"], &[]);
        let pid = await_pid(&proc).await;
        println!("PID={pid}");
        tokio::time::sleep(Duration::from_secs(600)).await;
        return;
    }

    assert_service_dies_with_its_killed_parent(SPAWN_TEST).await;
}

#[tokio::test]
async fn a_killed_test_process_does_not_leave_its_run_once_process_running() {
    if std::env::var_os(ORPHAN_CHILD).is_some() {
        let pid_file =
            std::env::temp_dir().join(format!("br-harness-run-once-pid-{}", std::process::id()));
        let path = pid_file.to_string_lossy().into_owned();
        let envs = [("PID_FILE", path.as_str())];
        let run = run_once(
            "sh",
            &["-c", "echo $$ > \"$PID_FILE\"; exec sleep 600"],
            &envs,
            Duration::from_secs(600),
        );
        let announce = async {
            let pid = await_pid_file(&pid_file).await;
            println!("PID={pid}");
            tokio::time::sleep(Duration::from_secs(600)).await;
        };
        tokio::select! {
            _ = run => panic!("the run_once process ended on its own"),
            _ = announce => {}
        }
        return;
    }

    assert_service_dies_with_its_killed_parent(RUN_ONCE_TEST).await;
}

async fn await_pid_file(path: &std::path::Path) -> u32 {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(pid) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the run_once process never wrote its pid"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn await_pid(proc: &SpawnedProcess) -> u32 {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(pid) = proc
            .logs()
            .lines()
            .find_map(|line| line.strip_prefix("PID=").and_then(|p| p.parse().ok()))
        {
            return pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the spawned process never printed its pid; logs:\n{}",
            proc.logs()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn assert_service_dies_with_its_killed_parent(test: &str) {
    let mut child = std::process::Command::new(std::env::current_exe().expect("current exe"))
        .args(["--exact", "--nocapture", "--test-threads=1", test])
        .env(ORPHAN_CHILD, "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn the child test process");
    let pid = read_pid(child.stdout.take().expect("child stdout")).await;
    assert!(
        is_alive(pid),
        "the child's process must be up before the child is killed"
    );

    child.kill().expect("SIGKILL the child test process");
    child.wait().expect("reap the child test process");
    let _ = std::fs::remove_file(
        std::env::temp_dir().join(format!("br-harness-run-once-pid-{}", child.id())),
    );

    let stopped = tokio::time::timeout(Duration::from_secs(15), async {
        while is_alive(pid) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if stopped.is_err() {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
    }
    assert!(
        stopped.is_ok(),
        "a process whose test process was killed must not keep running (pid {pid})"
    );
}

fn is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn read_pid(stdout: std::process::ChildStdout) -> u32 {
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead as _;
        std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .find_map(|line| {
                line.split_once("PID=")
                    .and_then(|(_, pid)| pid.parse().ok())
            })
            .expect("the child test process must print its service pid")
    })
    .await
    .expect("read the child's pid")
}
