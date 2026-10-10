#[path = "support/diagnostics.rs"]
mod diagnostics;
#[path = "support/tempdir.rs"]
mod fixtures;

use diagnostics::Diagnostics;
use herdr_idle_inhibitor::runtime::{
    ipc::{self, Operation},
    paths::Paths,
};
use std::{process::Command, time::Duration};
#[tokio::test]
async fn actual_status_is_read_only_and_concurrent_processes_have_one_owner() {
    let mut diagnostics = Diagnostics::new(fixtures::tempdir(), None);
    let directory = diagnostics.path().to_owned();
    let binary = env!("CARGO_BIN_EXE_herdr-idle-inhibitor");
    let command = || {
        let mut c = Command::new(binary);
        c.env("XDG_CONFIG_HOME", directory.as_path().join("config"))
            .env("XDG_STATE_HOME", directory.as_path().join("state"));
        c.env(
            "HERDR_IDLE_INHIBITOR_CONFIG",
            directory.as_path().join("config/config.toml"),
        )
        .env(
            "HERDR_IDLE_INHIBITOR_STATE",
            directory.as_path().join("state"),
        );
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
    diagnostics.record("initial_status", serde_json::json!({"exit":before.status.code(), "stdout":String::from_utf8_lossy(&before.stdout), "stderr":String::from_utf8_lossy(&before.stderr)}));
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
    assert!(!directory.as_path().join("config").exists());
    assert!(!directory.as_path().join("state").exists());
    #[cfg(unix)]
    {
        // A fully contextual SHOW must not bootstrap an owner; context is optional too.
        check_popup_open_path(directory.as_path(), &command);
        let after_show = command().args(["status", "--json"]).output().unwrap();
        assert_eq!(after_show.status.code(), Some(3));
        assert!(!directory.as_path().join("config").exists());
        assert!(!directory.as_path().join("state").exists());
        let without_registration = command()
            .arg("_open")
            .env("HERDR_BIN_PATH", "/usr/bin/true")
            .env_remove("HERDR_SOCKET_PATH")
            .env_remove("HERDR_PLUGIN_ROOT")
            .output()
            .unwrap();
        assert!(
            without_registration.status.success(),
            "{}",
            String::from_utf8_lossy(&without_registration.stderr)
        );
        assert!(!directory.as_path().join("config").exists());
        assert!(!directory.as_path().join("state").exists());
    }
    for _ in 0..4 {
        diagnostics
            .spawn("monitor", command().arg("_serve"))
            .unwrap();
    }
    let endpoint = Paths::get().unwrap().endpoint;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut attempts = 0;
    diagnostics.record("phase", serde_json::json!({"operation":"start four monitors; await one responding owner", "endpoint":endpoint,"deadline_seconds":5}));
    loop {
        attempts += 1;
        match ipc::query(&endpoint, Operation::GetStatus { details: false }).await {
            Ok(reply) => {
                diagnostics.record("status.last_reply", serde_json::json!(reply));
                break;
            }
            Err(error) => diagnostics.record("status.last_error", serde_json::json!({"endpoint":endpoint,"code":error.code,"exit":error.exit,"attempt":attempts})),
        }
        diagnostics.check(
            tokio::time::Instant::now() < deadline,
            "monitor startup deadline expired after 5 seconds",
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        diagnostics
            .children_mut()
            .iter_mut()
            .map(|c| c.try_wait().unwrap().is_none() as usize)
            .sum::<usize>(),
        1
    );
    for _ in 0..100 {
        let output = command().args(["status", "--json"]).output().unwrap();
        diagnostics.record("standalone_status", serde_json::json!({"exit":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
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
    diagnostics.record(
        "phase",
        serde_json::json!("verify passive SHOW, explicit START and shared Pause"),
    );
    let paused = ipc::query(&endpoint, Operation::SetPaused { paused: true })
        .await
        .unwrap();
    assert_eq!(paused.status["control"]["paused"], true);
    #[cfg(unix)]
    {
        let before_popup = ipc::query(&endpoint, Operation::GetStatus { details: true })
            .await
            .unwrap();
        check_popup_open_path(directory.as_path(), &command);
        let after_popup = ipc::query(&endpoint, Operation::GetStatus { details: true })
            .await
            .unwrap();
        assert_eq!(after_popup.status["control"]["paused"], true);
        for key in [
            "hints_received",
            "discovery_attempts",
            "snapshot_initial",
            "snapshot_poll",
            "snapshot_hook",
            "native_acquires",
            "native_releases",
        ] {
            assert_eq!(
                after_popup.status["diagnostics"]["counters"][key],
                before_popup.status["diagnostics"]["counters"][key],
                "SHOW induced {key}"
            );
        }
        assert!(
            after_popup
                .details
                .unwrap()
                .runtime
                .unwrap()
                .registrations
                .is_empty()
        );

        // Only the separately named START action may register and refresh this root.
        let manifest: toml::Value = toml::from_str(include_str!("../herdr-plugin.toml")).unwrap();
        let start = manifest["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"].as_str() == Some("start"))
            .expect("an explicit start action must be available independently of SHOW");
        let arguments = directory.as_path().join("popup-arguments");
        std::fs::remove_file(&arguments).unwrap();
        let started = command()
            .args(
                start["command"].as_array().unwrap()[1..]
                    .iter()
                    .map(|arg| arg.as_str().unwrap()),
            )
            .env("HERDR_BIN_PATH", directory.as_path().join("fake-herdr"))
            .env("HERDR_SOCKET_PATH", directory.as_path().join("herdr.sock"))
            .env("HERDR_PLUGIN_ROOT", directory.as_path())
            .output()
            .unwrap();
        assert!(
            started.status.success(),
            "{}",
            String::from_utf8_lossy(&started.stderr)
        );
        assert!(!arguments.exists(), "START opened the popup");
        let after_start = ipc::query(&endpoint, Operation::GetStatus { details: true })
            .await
            .unwrap();
        assert_eq!(after_start.status["control"]["paused"], true);
        assert_eq!(after_start.status["control"]["pause_persisted"], true);
        assert_eq!(
            after_start.status["diagnostics"]["counters"]["hints_received"],
            1
        );
        assert_eq!(
            after_start.status["diagnostics"]["counters"]["native_acquires"],
            0
        );
        assert_eq!(
            after_start.status["diagnostics"]["counters"]["native_releases"],
            0
        );
        let registrations = after_start.details.unwrap().runtime.unwrap().registrations;
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].plugin_root, directory.as_path());
        assert_eq!(
            registrations[0].endpoint,
            directory.as_path().join("herdr.sock").to_str().unwrap()
        );
    }
    diagnostics.record(
        "phase",
        serde_json::json!("terminate owned monitors; verify unavailable status"),
    );
    for child in diagnostics.children_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let after = command().args(["status", "--json"]).output().unwrap();
    diagnostics.record("after_shutdown", serde_json::json!({"exit":after.status.code(),"stdout":String::from_utf8_lossy(&after.stdout),"stderr":String::from_utf8_lossy(&after.stderr)}));
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
