use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CHILD: &str = "CONFORMANCE_DIRECTORY_ISOLATED_CHILD";
pub const NATS_PIDS: &str = "CONFORMANCE_DIRECTORY_NATS_PIDS";

const STUB_VERSION_ONLY: &str = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"nats-server: v2.14.1\"; exit 0; fi\nexit 1\n";

pub enum NatsServer {
    ExitsAfterVersion,
    RealRecordingItsPid,
}

pub struct Isolation<'a> {
    pub label: &'a str,
    pub test: &'a str,
    pub ignored: bool,
    pub fabric_nats: &'a str,
    pub nats_server: NatsServer,
    pub envs: &'a [(&'a str, &'a str)],
}

pub fn write_executable(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod a stub");
}

fn real_nats_server() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join("nats-server"))
        .find(|candidate| candidate.is_file())
        .expect("a real nats-server must be on PATH for this real-infra test")
}

pub fn run(isolation: &Isolation) {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = crate::scratch::scratch(isolation.label);
    let deps = isolated.join("deps");
    let stubs = isolated.join("stubs");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    std::fs::create_dir_all(&stubs).expect("create the stub directory");
    write_executable(&isolated.join("fabric-nats"), isolation.fabric_nats);
    let pids = isolated.join("nats-pids");
    let nats_server = match isolation.nats_server {
        NatsServer::ExitsAfterVersion => STUB_VERSION_ONLY.to_string(),
        NatsServer::RealRecordingItsPid => format!(
            "#!/bin/sh\nif [ \"$1\" != \"--version\" ]; then echo $$ >> \"${NATS_PIDS}\"; fi\nexec {} \"$@\"\n",
            real_nats_server().display(),
        ),
    };
    write_executable(&stubs.join("nats-server"), &nats_server);
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let path = std::env::join_paths([&*stubs, Path::new("/usr/bin"), Path::new("/bin")])
        .expect("join the PATH");
    let mut command = Command::new(&copy);
    command.args(["--exact", isolation.test, "--nocapture"]);
    if isolation.ignored {
        command.arg("--ignored");
    }
    command
        .env(CHILD, "1")
        .env(NATS_PIDS, &pids)
        .env("PATH", path);
    for (name, value) in isolation.envs {
        command.env(name, value);
    }
    let output = command.output().expect("run the isolated copy");
    crate::child::assert_child_ran_one_passing_test(&output);
}

pub fn assert_every_started_nats_server_is_gone() {
    let pids_file = std::env::var(NATS_PIDS).expect("the pid file of the recording nats-server");
    let recorded =
        std::fs::read_to_string(&pids_file).expect("a nats-server must have been started");
    let pids: Vec<&str> = recorded.split_whitespace().collect();
    assert!(
        !pids.is_empty(),
        "the throwaway nats-server was never started"
    );
    for pid in pids {
        let mut alive = true;
        for _ in 0..100 {
            alive = Command::new("kill")
                .args(["-0", pid])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!alive, "the throwaway nats-server {pid} is still running");
    }
}
