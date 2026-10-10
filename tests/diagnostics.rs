#[path = "support/diagnostics.rs"]
mod diagnostics;
#[path = "support/tempdir.rs"]
mod fixtures;

use diagnostics::Diagnostics;
use herdr_idle_inhibitor::runtime::ipc::{self, Operation};
use interprocess::local_socket::tokio::prelude::*;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

// The same test executable is the portable child: no shell or extra compiler is required.
#[test]
fn diagnostic_child() {
    let Ok(mode) = std::env::var("DIAGNOSTIC_TEST_MODE") else {
        return;
    };
    println!("stdout diagnostic canary");
    eprintln!("stderr diagnostic canary");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();
    if mode == "exit" {
        std::process::exit(23);
    }
    assert_eq!(mode, "stall");
    let mut input =
        std::fs::File::open(std::env::var_os("DIAGNOSTIC_TEST_INPUT").unwrap()).unwrap();
    let mut bytes = [0; 17];
    input.read_exact(&mut bytes).unwrap();
    std::fs::write(std::env::var_os("DIAGNOSTIC_TEST_READY").unwrap(), "ready").unwrap();
    loop {
        std::hint::black_box(&input);
        std::thread::park_timeout(Duration::from_secs(1));
    }
}
fn child(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "diagnostic_child", "--nocapture"])
        .env("DIAGNOSTIC_TEST_MODE", mode);
    command
}
fn report(directory: &Path) -> (std::path::PathBuf, Value) {
    let entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "one failure must create exactly one artifact"
    );
    let data =
        serde_json::from_slice(&std::fs::read(entries[0].join("report.json")).unwrap()).unwrap();
    (entries[0].clone(), data)
}
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        payload.downcast_ref::<&str>().unwrap().to_string()
    }
}
#[test]
fn early_exit_preserves_stderr_exit_code_artifact_identity_and_last_error() {
    let artifacts = fixtures::tempdir();
    let dir = fixtures::tempdir();
    let path = dir.path().to_owned();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut diagnostics = Diagnostics::new(dir, Some(artifacts.path().to_owned()));
        diagnostics.record(
            "last_query",
            json!({"endpoint":"fixture-absent", "code":"monitor_unavailable", "exit":3}),
        );
        diagnostics.spawn("monitor", &mut child("exit")).unwrap();
        assert_eq!(
            diagnostics.children_mut()[0].wait().unwrap().code(),
            Some(23)
        );
        diagnostics.check(false, "monitor startup deadline expired");
    }))
    .unwrap_err();
    assert!(panic_message(failure).contains("monitor startup deadline expired"));
    assert!(
        !path.exists(),
        "normal temp cleanup still occurs after evidence is copied"
    );
    let (artifact, data) = report(artifacts.path());
    assert_eq!(data["reason"], "monitor startup deadline expired");
    assert_eq!(data["context"]["last_query"]["code"], "monitor_unavailable");
    assert_eq!(data["processes"][0]["exit_code"], 23);
    assert_eq!(
        data["binaries"][0]["bytes"],
        std::fs::metadata(std::env::current_exe().unwrap())
            .unwrap()
            .len()
    );
    assert_eq!(data["binaries"][0]["sha256"].as_str().unwrap().len(), 64);
    assert!(
        std::fs::read_to_string(artifact.join("files/processes/0-monitor.stderr"))
            .unwrap()
            .contains("stderr diagnostic canary")
    );
    assert!(
        std::fs::read_to_string(artifact.join("files/processes/0-monitor.stdout"))
            .unwrap()
            .contains("stdout diagnostic canary")
    );
}
#[tokio::test]
async fn stalled_ipc_captures_alive_process_fds_last_timeout_and_fixture_markers() {
    let artifacts = fixtures::tempdir();
    let dir = fixtures::tempdir();
    let mut diagnostics = Diagnostics::new(dir, Some(artifacts.path().to_owned()));
    let input = diagnostics.path().join("held.bin");
    let ready = diagnostics.path().join("child-ready");
    std::fs::write(&input, [42; 100]).unwrap();
    let mut command = child("stall");
    command
        .env("DIAGNOSTIC_TEST_INPUT", &input)
        .env("DIAGNOSTIC_TEST_READY", &ready);
    diagnostics.spawn("monitor", &mut command).unwrap();
    let pid = diagnostics.children_mut()[0].id();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.is_file() {
        assert!(
            Instant::now() < deadline,
            "diagnostic child did not become ready"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    #[cfg(unix)]
    let endpoint = diagnostics
        .path()
        .join("stalled.sock")
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    let endpoint = format!("diagnostic-stalled-{}", uuid::Uuid::new_v4());
    let listener = ipc::test_listener(&endpoint).unwrap();
    let peer = tokio::spawn(async move {
        let _stream = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let error = ipc::query(&endpoint, Operation::GetStatus { details: true })
        .await
        .unwrap_err();
    assert_eq!(error.code, "query_timeout");
    diagnostics.record(
        "last_query",
        json!({"endpoint":endpoint, "code":error.code, "exit":error.exit}),
    );
    std::fs::create_dir_all(diagnostics.path().join("state/updates/ticket")).unwrap();
    std::fs::write(
        diagnostics.path().join("state/updates/ticket/error.json"),
        r#"{"error":"publication not observed"}"#,
    )
    .unwrap();
    let failure = catch_unwind(AssertUnwindSafe(move || {
        diagnostics.check(false, "stalled monitor response deadline")
    }))
    .unwrap_err();
    assert!(panic_message(failure).contains("stalled monitor response deadline"));
    peer.abort();
    let (artifact, data) = report(artifacts.path());
    assert_eq!(data["processes"][0]["pid"], pid);
    assert_eq!(data["processes"][0]["state"], "running");
    assert_eq!(data["context"]["last_query"]["code"], "query_timeout");
    assert!(
        artifact
            .join("files/state/updates/ticket/error.json")
            .is_file()
    );
    #[cfg(target_os = "linux")]
    {
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "the diagnostic must not leak its stalled child"
        );
        let fds = data["processes"][0]["os"]["fds"].as_array().unwrap();
        let descriptor = fds
            .iter()
            .find(|fd| fd["target"] == input.to_string_lossy().as_ref())
            .unwrap();
        assert!(descriptor["info"].as_str().unwrap().contains("pos:\t17"));
        assert!(data["processes"][0]["os"]["user_ticks"].is_number());
    }
}
#[test]
fn ordinary_assertion_preserves_useful_files_but_never_binaries_or_symlink_targets() {
    let artifacts = fixtures::tempdir();
    let external = fixtures::tempdir();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let diagnostics = Diagnostics::new(fixtures::tempdir(), Some(artifacts.path().to_owned()));
        std::fs::write(diagnostics.path().join("config.toml"), "paused = true\n").unwrap();
        std::fs::write(diagnostics.path().join("binary.exe"), [5; 100]).unwrap();
        std::fs::write(
            external.path().join("private.json"),
            "external secret must not be copied",
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(external.path(), diagnostics.path().join("external")).unwrap();
        diagnostics.record("phase", json!("asserting rollback result"));
        panic!("ordinary regression assertion");
    }))
    .unwrap_err();
    assert_eq!(panic_message(failure), "ordinary regression assertion");
    let (artifact, data) = report(artifacts.path());
    assert_eq!(data["context"]["phase"], "asserting rollback result");
    assert!(artifact.join("files/config.toml").is_file());
    assert!(!artifact.join("files/binary.exe").exists());
    assert!(!artifact.join("files/external/private.json").exists());
}
#[test]
fn success_cleans_fixture_and_creates_no_failure_artifact() {
    let artifacts = fixtures::tempdir();
    let path;
    {
        let mut diagnostics =
            Diagnostics::new(fixtures::tempdir(), Some(artifacts.path().to_owned()));
        path = diagnostics.path().to_owned();
        diagnostics.record("phase", json!("successful cleanup"));
        diagnostics.check(true, "must not emit a failure");
    }
    assert!(!path.exists());
    assert_eq!(std::fs::read_dir(artifacts.path()).unwrap().count(), 0);
}
#[test]
fn unavailable_artifact_destination_cannot_replace_the_original_failure() {
    let outer = fixtures::tempdir();
    let blocked = outer.path().join("not-a-directory");
    std::fs::write(&blocked, "block artifact storage").unwrap();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut diagnostics = Diagnostics::new(fixtures::tempdir(), Some(blocked));
        diagnostics.check(false, "original deadline failure");
    }))
    .unwrap_err();
    assert!(panic_message(failure).contains("original deadline failure"));
}

#[test]
fn oversized_logs_are_capped_and_truncation_is_explicit() {
    let artifacts = fixtures::tempdir();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut diagnostics =
            Diagnostics::new(fixtures::tempdir(), Some(artifacts.path().to_owned()));
        let mut bytes = vec![b'x'; 600 * 1024];
        bytes.extend_from_slice(b"tail canary");
        std::fs::write(diagnostics.path().join("large.log"), bytes).unwrap();
        diagnostics.check(false, "log truncation test failure");
    }))
    .unwrap_err();
    assert!(panic_message(failure).contains("log truncation test failure"));
    let (artifact, data) = report(artifacts.path());
    let log = std::fs::read(artifact.join("files/large.log")).unwrap();
    assert_eq!(log.len(), 512 * 1024);
    assert!(log.ends_with(b"tail canary"));
    assert!(
        data["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("truncated tail"))
    );
}
