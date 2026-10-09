#[path = "support/tempdir.rs"]
mod fixtures;

use herdr_idle_inhibitor::runtime::{
    ipc::{self, Operation},
    paths::Paths,
};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for c in &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
#[tokio::test]
async fn actual_status_is_read_only_and_concurrent_processes_have_one_owner() {
    let dir = fixtures::tempdir();
    let binary = env!("CARGO_BIN_EXE_herdr-idle-inhibitor");
    let command = || {
        let mut c = Command::new(binary);
        c.env("XDG_CONFIG_HOME", dir.path().join("config"))
            .env("XDG_STATE_HOME", dir.path().join("state"));
        c.env(
            "HERDR_IDLE_INHIBITOR_CONFIG",
            dir.path().join("config/config.toml"),
        )
        .env("HERDR_IDLE_INHIBITOR_STATE", dir.path().join("state"));
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with("HERDR_")
                && !k.to_string_lossy().starts_with("HERDR_IDLE_INHIBITOR_")
            {
                c.env_remove(k);
            }
        }
        c
    };
    let before = command().args(["status", "--json"]).output().unwrap();
    assert_eq!(
        before.status.code(),
        Some(3),
        "This test requires no existing production monitor"
    );
    let value: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
    assert_eq!(
        value["observation"]["working_agents_observed"],
        serde_json::Value::Null
    );
    assert_eq!(value["available"], false);
    assert!(!dir.path().join("config").exists());
    assert!(!dir.path().join("state").exists());
    let mut children = Children(vec![]);
    for _ in 0..4 {
        children.0.push(
            command()
                .arg("_serve")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    let endpoint = Paths::get().unwrap().endpoint;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if ipc::query(&endpoint, Operation::GetStatus { details: false })
            .await
            .is_ok()
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        children
            .0
            .iter_mut()
            .map(|c| c.try_wait().unwrap().is_none() as usize)
            .sum::<usize>(),
        1
    );
    for _ in 0..100 {
        let output = command().args(["status", "--json"]).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout.iter().filter(|b| **b == b'\n').count(), 1);
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(v["observation"]["work"], "unknown");
        let counters = &v["diagnostics"]["counters"];
        for key in [
            "discovery_attempts",
            "snapshot_initial",
            "snapshot_poll",
            "snapshot_hook",
            "native_acquires",
            "native_releases",
        ] {
            assert_eq!(counters[key], 0, "status induced {key}");
        }
    }
    let paused = ipc::query(&endpoint, Operation::SetPaused { paused: true })
        .await
        .unwrap();
    assert_eq!(paused.status["control"]["paused"], true);
    #[cfg(unix)]
    {
        check_popup_open_path(dir.path(), &command);
        let after_popup = ipc::query(&endpoint, Operation::GetStatus { details: false })
            .await
            .unwrap();
        assert_eq!(after_popup.status["control"]["paused"], true);
        assert_eq!(
            after_popup.status["diagnostics"]["counters"]["native_acquires"],
            0
        );
        assert_eq!(
            after_popup.status["diagnostics"]["counters"]["native_releases"],
            0
        );
    }
    drop(children);
    let after = command().args(["status", "--json"]).output().unwrap();
    assert_eq!(after.status.code(), Some(3));
}

#[cfg(windows)]
#[test]
fn source_manifest_extensionless_windows_entrypoint_resolves_native_exe() {
    let binary = env!("CARGO_BIN_EXE_herdr-idle-inhibitor");
    let output = std::process::Command::new(binary.strip_suffix(".exe").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("herdr-idle-inhibitor {}", env!("CARGO_PKG_VERSION"))
    );
}

#[cfg(unix)]
fn check_popup_open_path(directory: &std::path::Path, command: &impl Fn() -> Command) {
    use std::os::unix::fs::PermissionsExt;

    let herdr = directory.join("fake-herdr");
    let arguments = directory.join("popup-arguments");
    std::fs::write(
        &herdr,
        r#"#!/bin/sh
if [ "$1 $2 $3" != "plugin pane open" ]; then
    exit 1
fi
printf '%s\n' "$@" > "$POPUP_TEST_ARGUMENTS"
if [ "$#" -ne 7 ] || [ "$4" != "--plugin" ] || \
   [ "$5" != "herdr-idle-inhibitor" ] || [ "$6" != "--entrypoint" ] || \
   [ "$7" != "status" ]; then
    printf '%s\n' 'fixture: Herdr requires --plugin and --entrypoint' >&2
    exit 2
fi
if [ "$POPUP_TEST_EXIT" != "0" ]; then
    printf '%s\n' 'fixture: popup request rejected' >&2
fi
exit "$POPUP_TEST_EXIT"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&herdr, std::fs::Permissions::from_mode(0o700)).unwrap();
    let invoke = |exit: &str| {
        command()
            .arg("_open")
            .env("HERDR_BIN_PATH", &herdr)
            .env("HERDR_SOCKET_PATH", directory.join("herdr.sock"))
            .env("HERDR_PLUGIN_ROOT", directory)
            .env("POPUP_TEST_ARGUMENTS", &arguments)
            .env("POPUP_TEST_EXIT", exit)
            .output()
            .unwrap()
    };

    let success = invoke("0");
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&arguments)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        [
            "plugin",
            "pane",
            "open",
            "--plugin",
            "herdr-idle-inhibitor",
            "--entrypoint",
            "status",
        ]
    );

    let rejected = invoke("7");
    assert!(!rejected.status.success());
    let error = String::from_utf8(rejected.stderr).unwrap();
    assert!(error.contains("fixture: popup request rejected"), "{error}");
    assert!(
        error.contains("Herdr popup command failed (exit status: 7)"),
        "{error}"
    );
    assert!(!error.contains("another popup may be open"), "{error}");
    assert!(!error.contains("no Herdr UI is attached"), "{error}");

    // The file still passes registration validation, but its missing interpreter
    // makes process creation fail even when the test is run as root.
    std::fs::write(&herdr, "#!/herdr-popup-test-missing-interpreter\n").unwrap();
    let not_executable = invoke("0");
    assert!(!not_executable.status.success());
    let error = String::from_utf8(not_executable.stderr).unwrap();
    assert!(
        error.contains("Cannot execute Herdr popup command"),
        "{error}"
    );
    assert!(
        error.contains(&format!("os error {}", libc::ENOENT)),
        "{error}"
    );
}
