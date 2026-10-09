//! Real cached executables and update helpers against captured Herdr traffic.
//! Linux overlay bytes produce different artifacts with the same package version.
#![cfg(target_os = "linux")]
#[path = "support/tempdir.rs"]
mod fixtures;
use herdr_idle_inhibitor::{
    herdr::discovery::Registration,
    runtime::{
        cache,
        ipc::{self, Operation},
        paths::Paths,
        update::{Handoff, Plan, Publication},
    },
};
use interprocess::local_socket::tokio::prelude::*;
use std::{
    collections::BTreeMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::AsyncWriteExt;

struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    root: PathBuf,
    candidate: cache::Binary,
    old: cache::Binary,
    registration: Registration,
    snapshots: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
    children: Vec<Child>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "worker log: {:?}",
                std::fs::read_to_string(self.paths.state.join("monitor.log"))
            );
            if let Ok(entries) = std::fs::read_dir(self.paths.state.join("updates")) {
                for entry in entries.flatten() {
                    eprintln!(
                        "worker error: {:?}",
                        std::fs::read_to_string(entry.path().join("error.json"))
                    );
                }
            }
        }
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.server.abort();
    }
}
fn private_json(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
async fn status(endpoint: &str, test: impl Fn(&serde_json::Value) -> bool) -> ipc::Reply {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(reply) = ipc::query(endpoint, Operation::GetStatus { details: true }).await
            && test(&reply.status)
        {
            return reply;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "status replay deadline expired"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn marker(path: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !path.is_file() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "update marker deadline: {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
impl Fixture {
    async fn new() -> Self {
        let dir = fixtures::tempdir();
        let root = dir.path().join("installed");
        let executable = cache::installed(&root);
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        let artifact = dir.path().join("application");
        std::fs::copy(env!("CARGO_BIN_EXE_herdr-idle-inhibitor"), &artifact).unwrap();
        // Cargo's 100+ MiB debug symbols are not runtime code. Remove only symbols
        // before repeatedly hashing real executables in concurrent process fixtures.
        assert!(
            Command::new("strip")
                .arg("--strip-debug")
                .arg(&artifact)
                .status()
                .unwrap()
                .success()
        );
        let mut bytes = std::fs::read(&artifact).unwrap();
        bytes.extend_from_slice(b"old artifact overlay");
        std::fs::write(&executable, bytes).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let paths = Paths {
            config: dir.path().join("config.toml"),
            state: dir.path().join("state"),
            runtime: dir.path().join("runtime"),
            endpoint: dir
                .path()
                .join("control.sock")
                .to_string_lossy()
                .into_owned(),
        };
        // Never acquire a real native power request in these process tests.
        let config = herdr_idle_inhibitor::runtime::config::Config {
            paused: true,
            release_delay_secs: 17,
            ..Default::default()
        };
        std::fs::write(&paths.config, toml::to_string(&config).unwrap()).unwrap();
        let old = cache::stage(&executable, &paths.state).unwrap();
        let candidate = cache::stage(&artifact, &paths.state).unwrap();
        assert_ne!(old.digest, candidate.digest);
        let herdr = dir.path().join("herdr");
        assert!(
            Command::new("rustc")
                .args(["tests/support/fake_cli.rs", "-o"])
                .arg(&herdr)
                .status()
                .unwrap()
                .success()
        );
        let session = dir.path().join("herdr.sock").to_string_lossy().into_owned();
        let sessions = dir.path().join("sessions.json");
        std::fs::write(&sessions, serde_json::json!({"sessions":[{"name":"default","running":true,"socket_path":session}]}).to_string()).unwrap();
        let registration = Registration {
            endpoint: session.clone(),
            herdr_bin: herdr,
            plugin_root: root.clone(),
            env: BTreeMap::from([(
                "HERDR_CONFIG_PATH".into(),
                sessions.to_string_lossy().into_owned(),
            )]),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        };
        let plugin = serde_json::json!({"plugin_id":"herdr-idle-inhibitor","enabled":true,"plugin_root":root,"source":{"kind":"github","resolved_commit":"old-commit"}});
        private_json(
            &PathBuf::from(format!("{}.plugins", sessions.display())),
            &serde_json::json!({"result":{"plugins":[plugin.clone()]}}),
        );
        let listener = ipc::test_listener(&session).unwrap();
        let snapshots = Arc::new(AtomicUsize::new(0));
        let counter = snapshots.clone();
        let server = tokio::spawn(async move {
            loop {
                let Ok(mut stream) = listener.accept().await else {
                    break;
                };
                let plugin = plugin.clone();
                let counter = counter.clone();
                tokio::spawn(async move {
                    let frame = herdr_idle_inhibitor::herdr::transport::read_frame(
                        &mut stream,
                        1024 * 1024,
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    let request: serde_json::Value = serde_json::from_slice(&frame).unwrap();
                    let id = request["id"].clone();
                    let value = match request["method"].as_str().unwrap() {
                        "plugin.list" => {
                            serde_json::json!({"id":id,"result":{"type":"plugin_list","plugins":[plugin]}})
                        }
                        "ping" => {
                            let mut v: serde_json::Value = serde_json::from_str(include_str!(
                                "fixtures/herdr-0.9.3/ping.json"
                            ))
                            .unwrap();
                            v["id"] = id;
                            v
                        }
                        "agent.list" => {
                            counter.fetch_add(1, Ordering::SeqCst);
                            let mut v: serde_json::Value = serde_json::from_str(include_str!(
                                "fixtures/herdr-0.9.3/agent-list.json"
                            ))
                            .unwrap();
                            v["id"] = id;
                            v
                        }
                        method => panic!("unexpected Herdr method {method}"),
                    };
                    let mut bytes = serde_json::to_vec(&value).unwrap();
                    bytes.push(b'\n');
                    stream.write_all(&bytes).await.unwrap();
                });
            }
        });
        let boot = Handoff {
            paths: paths.clone(),
            registrations: vec![registration.clone()],
            legacy_endpoints: vec![],
            effective_pause: None,
            source: Some(executable),
            rejected_updates: Default::default(),
        };
        let boot_path = dir.path().join("boot.json");
        private_json(&boot_path, &boot);
        let child = Command::new(&old.path)
            .env("TOKIO_WORKER_THREADS", "2")
            .arg("_resume")
            .arg(&boot_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        status(&paths.endpoint, |v| v["observation"]["work"] == "working").await;
        Self {
            _dir: dir,
            paths,
            root,
            candidate,
            old,
            registration,
            snapshots,
            server,
            children: vec![child],
        }
    }
    fn plan(&self, timeout_ms: u64) -> Plan {
        Plan {
            paths: self.paths.clone(),
            candidate: self.candidate.clone(),
            publication: Publication {
                executable: cache::installed(&self.root),
                digest: self.candidate.digest.clone(),
                plugin_root: self.root.clone(),
                expected_commit: Some("new-commit".into()),
            },
            base: self.registration.clone(),
            handoff: None,
            prepare: true,
            direct: false,
            discover_root: false,
            source_root: None,
            timeout_ms,
        }
    }
    fn worker(&mut self, plan: &Plan) -> PathBuf {
        let directory = self
            ._dir
            .path()
            .join(format!("worker-{}", self.children.len()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("plan.json");
        private_json(&path, plan);
        let child = Command::new(&self.old.path)
            .env("TOKIO_WORKER_THREADS", "2")
            .arg("_upgrade_worker")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.children.push(child);
        directory
    }
    fn publish(&self, commit: &str) {
        let target = cache::installed(&self.root);
        let replacement = target.with_extension("new");
        std::fs::copy(&self.candidate.path, &replacement).unwrap();
        std::fs::rename(replacement, target).unwrap();
        private_json(
            &PathBuf::from(format!(
                "{}.plugins",
                self.registration.env["HERDR_CONFIG_PATH"]
            )),
            &serde_json::json!({"result":{"plugins":[{"plugin_id":"herdr-idle-inhibitor","enabled":true,"plugin_root":self.root,"source":{"kind":"github","resolved_commit":commit}}]}}),
        );
    }
}

#[tokio::test]
async fn cached_monitor_remains_live_until_both_files_and_registry_publish_then_restarts() {
    let mut fixture = Fixture::new().await;
    let original = std::fs::read(&fixture.paths.config).unwrap();
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let instance = before.status["monitor"]["instance_id"].clone();
    let directory = fixture.worker(&fixture.plan(5000));
    marker(&directory.join("ready.json")).await;
    assert_eq!(
        status(&fixture.paths.endpoint, |_| true).await.status["monitor"]["instance_id"],
        instance
    );
    fixture.publish("old-commit");
    assert_eq!(
        status(&fixture.paths.endpoint, |_| true).await.status["monitor"]["instance_id"],
        instance,
        "file replacement alone cannot terminate the working monitor"
    );
    let snapshots = fixture.snapshots.load(Ordering::SeqCst);
    fixture.publish("new-commit");
    marker(&directory.join("complete.json")).await;
    let after = status(&fixture.paths.endpoint, |v| {
        v["observation"]["work"] == "working"
    })
    .await;
    assert_ne!(after.status["monitor"]["instance_id"], instance);
    assert_eq!(
        after.status["monitor"]["app_version"],
        before.status["monitor"]["app_version"]
    );
    assert_eq!(
        after.details.unwrap().runtime.unwrap().digest,
        fixture.candidate.digest
    );
    assert_eq!(after.status["control"]["paused"], true);
    assert_eq!(
        after.status["diagnostics"]["counters"]["native_acquires"],
        0
    );
    assert!(
        fixture.snapshots.load(Ordering::SeqCst) > snapshots,
        "new process must replay real captured Herdr traffic"
    );
    assert_eq!(std::fs::read(&fixture.paths.config).unwrap(), original);
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_publication_keeps_the_old_cached_monitor_and_settings() {
    let mut fixture = Fixture::new().await;
    let original = std::fs::read(&fixture.paths.config).unwrap();
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let directory = fixture.worker(&fixture.plan(1000));
    marker(&directory.join("ready.json")).await;
    let mut queued = fixture.registration.clone();
    queued.env.insert(
        "XDG_STATE_HOME".into(),
        fixture
            ._dir
            .path()
            .join("queued-root")
            .to_string_lossy()
            .into_owned(),
    );
    assert!(
        herdr_idle_inhibitor::runtime::update::queue_if_pending(&fixture.paths, &queued)
            .await
            .unwrap()
    );
    marker(&directory.join("error.json")).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let after = status(&fixture.paths.endpoint, |_| true).await;
    assert_eq!(
        after.status["monitor"]["instance_id"],
        before.status["monitor"]["instance_id"]
    );
    assert_eq!(
        after
            .details
            .as_ref()
            .unwrap()
            .runtime
            .as_ref()
            .unwrap()
            .digest,
        fixture.old.digest
    );
    assert!(
        after
            .details
            .unwrap()
            .runtime
            .unwrap()
            .registrations
            .contains(&queued),
        "failed publication must replay queued activation to the surviving owner"
    );
    assert_eq!(std::fs::read(&fixture.paths.config).unwrap(), original);
    assert!(!herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap());
}

#[tokio::test]
async fn identical_artifact_does_not_restart_the_cached_owner() {
    let mut fixture = Fixture::new().await;
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let mut plan = fixture.plan(5000);
    plan.candidate = fixture.old.clone();
    plan.publication.digest = fixture.old.digest.clone();
    let directory = fixture.worker(&plan);
    marker(&directory.join("ready.json")).await;
    private_json(
        &PathBuf::from(format!(
            "{}.plugins",
            fixture.registration.env["HERDR_CONFIG_PATH"]
        )),
        &serde_json::json!({"result":{"plugins":[{"plugin_id":"herdr-idle-inhibitor","enabled":true,"plugin_root":fixture.root,"source":{"kind":"github","resolved_commit":"new-commit"}}]}}),
    );
    marker(&directory.join("complete.json")).await;
    let after = status(&fixture.paths.endpoint, |_| true).await;
    assert_eq!(
        after.status["monitor"]["instance_id"],
        before.status["monitor"]["instance_id"]
    );
    assert_eq!(
        after.status["diagnostics"]["counters"]["native_acquires"],
        before.status["diagnostics"]["counters"]["native_acquires"]
    );
    assert_eq!(
        after.status["diagnostics"]["counters"]["native_releases"],
        before.status["diagnostics"]["counters"]["native_releases"]
    );
}

#[tokio::test]
async fn activation_during_publication_is_replayed_before_the_gate_opens() {
    let mut fixture = Fixture::new().await;
    let directory = fixture.worker(&fixture.plan(5000));
    marker(&directory.join("ready.json")).await;
    let mut queued = fixture.registration.clone();
    queued.env.insert(
        "XDG_STATE_HOME".into(),
        fixture
            ._dir
            .path()
            .join("another-root-state")
            .to_string_lossy()
            .into_owned(),
    );
    assert!(
        herdr_idle_inhibitor::runtime::update::queue_if_pending(&fixture.paths, &queued)
            .await
            .unwrap()
    );
    fixture.publish("new-commit");
    marker(&directory.join("complete.json")).await;
    let after = status(&fixture.paths.endpoint, |v| {
        v["observation"]["work"] == "working"
    })
    .await;
    assert!(
        after
            .details
            .unwrap()
            .runtime
            .unwrap()
            .registrations
            .contains(&queued)
    );
    assert!(!herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap());
    assert!(
        !herdr_idle_inhibitor::runtime::update::queue_if_pending(&fixture.paths, &queued)
            .await
            .unwrap()
    );
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn source_build_preparation_returns_before_publication_and_uses_registered_root() {
    let fixture = Fixture::new().await;
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let checkout = fixture._dir.path().join("new-checkout");
    let source = cache::installed(&checkout);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::copy(&fixture.candidate.path, &source).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    tokio::time::timeout_at(
        deadline,
        herdr_idle_inhibitor::runtime::update::prepare_from(
            fixture.paths.clone(),
            source,
            fixture.registration.clone(),
            checkout,
            Some("new-commit".into()),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap());
    assert_eq!(
        status(&fixture.paths.endpoint, |_| true).await.status["monitor"]["instance_id"],
        before.status["monitor"]["instance_id"]
    );
    fixture.publish("new-commit");
    let after = status(&fixture.paths.endpoint, |v| {
        v["monitor"]["instance_id"] != before.status["monitor"]["instance_id"]
            && v["observation"]["work"] == "working"
    })
    .await;
    assert_eq!(
        after.details.unwrap().runtime.unwrap().digest,
        fixture.candidate.digest
    );
    assert_eq!(after.status["control"]["paused"], true);
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn first_install_discovers_the_published_custom_root_without_a_hardcoded_config_path() {
    let fixture = Fixture::new().await;
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while ipc::query(
        &fixture.paths.endpoint,
        Operation::GetStatus { details: false },
    )
    .await
    .is_ok()
    {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let registry = PathBuf::from(format!(
        "{}.plugins",
        fixture.registration.env["HERDR_CONFIG_PATH"]
    ));
    private_json(&registry, &serde_json::json!({"result":{"plugins":[]}}));
    let checkout = fixture._dir.path().join("new-checkout");
    let source = cache::installed(&checkout);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::copy(&fixture.candidate.path, &source).unwrap();
    let mut base = fixture.registration.clone();
    base.plugin_root = checkout.clone();
    herdr_idle_inhibitor::runtime::update::prepare_from(
        fixture.paths.clone(),
        source,
        base,
        checkout,
        Some("new-commit".into()),
    )
    .await
    .unwrap();
    assert!(herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap());
    fixture.publish("new-commit");
    let after = status(&fixture.paths.endpoint, |v| {
        v["observation"]["work"] == "working"
    })
    .await;
    assert!(
        after
            .details
            .unwrap()
            .runtime
            .unwrap()
            .registrations
            .iter()
            .any(|r| r.plugin_root == fixture.root)
    );
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_new_process_restores_the_previous_cached_artifact_and_pause() {
    let mut fixture = Fixture::new().await;
    let config = std::fs::read(&fixture.paths.config).unwrap();
    let bad = fixture._dir.path().join("bad-application");
    std::fs::write(&bad, "#!/bin/sh\ncase \"$1\" in\n--version) echo 'herdr-idle-inhibitor 0.1.0';;\n_capabilities) echo '{\"update_protocol\":1}';;\n*) exit 77;;\nesac\n").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o700)).unwrap();
    fixture.candidate = cache::stage(&bad, &fixture.paths.state).unwrap();
    let directory = fixture.worker(&fixture.plan(5000));
    marker(&directory.join("ready.json")).await;
    fixture.publish("new-commit");
    marker(&directory.join("error.json")).await;
    let after = status(&fixture.paths.endpoint, |v| {
        v["observation"]["work"] == "working"
    })
    .await;
    assert_eq!(
        after.details.unwrap().runtime.unwrap().digest,
        fixture.old.digest
    );
    assert_eq!(after.status["control"]["paused"], true);
    assert_eq!(
        after.status["diagnostics"]["counters"]["native_acquires"],
        0
    );
    assert_eq!(std::fs::read(&fixture.paths.config).unwrap(), config);
    assert!(!directory.join("complete.json").exists());
    let instance = after.status["monitor"]["instance_id"].clone();
    tokio::time::sleep(Duration::from_secs(11)).await; // Two ordinary replacement scans.
    assert_eq!(
        status(&fixture.paths.endpoint, |_| true).await.status["monitor"]["instance_id"],
        instance,
        "recovery must not repeatedly retry the same failed artifact"
    );
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn resident_monitor_detects_same_version_file_replacement_without_herdr_events() {
    let fixture = Fixture::new().await;
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let config = std::fs::read(&fixture.paths.config).unwrap();
    fixture.publish("new-commit"); // No startup, event hook or prepare-worker call.
    let after = status(&fixture.paths.endpoint, |v| {
        v["monitor"]["instance_id"] != before.status["monitor"]["instance_id"]
            && v["observation"]["work"] == "working"
    })
    .await;
    assert_eq!(
        after.status["monitor"]["app_version"],
        before.status["monitor"]["app_version"]
    );
    assert_eq!(
        after.details.unwrap().runtime.unwrap().digest,
        fixture.candidate.digest
    );
    assert_eq!(std::fs::read(&fixture.paths.config).unwrap(), config);
    assert_eq!(after.status["control"]["paused"], true);
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_preparation_replays_activation_even_before_the_owner_was_described() {
    let mut fixture = Fixture::new().await;
    let entered = fixture._dir.path().join("probe-entered");
    let source = fixture._dir.path().join("slow-probe");
    std::fs::write(
        &source,
        format!(
            "#!/bin/sh\ntouch '{}'\nsleep 3\nexit 77\n",
            entered.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut plan = fixture.plan(5000);
    plan.candidate = cache::stage(&source, &fixture.paths.state).unwrap();
    plan.publication.digest = plan.candidate.digest.clone();
    let directory = fixture.worker(&plan);
    marker(&entered).await;
    let mut queued = fixture.registration.clone();
    queued.env.insert(
        "XDG_STATE_HOME".into(),
        fixture
            ._dir
            .path()
            .join("queued-during-probe")
            .to_string_lossy()
            .into_owned(),
    );
    assert!(
        herdr_idle_inhibitor::runtime::update::queue_if_pending(&fixture.paths, &queued)
            .await
            .unwrap()
    );
    marker(&directory.join("error.json")).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let reply = status(&fixture.paths.endpoint, |_| true).await;
    assert!(
        reply
            .details
            .unwrap()
            .runtime
            .unwrap()
            .registrations
            .contains(&queued)
    );
}

#[tokio::test]
async fn queued_activation_is_removed_only_after_a_live_owner_acknowledges_it() {
    let fixture = Fixture::new().await;
    let gate = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(fixture.paths.runtime.join("update.lock"))
        .unwrap();
    std::fs::set_permissions(
        fixture.paths.runtime.join("update.lock"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    gate.try_lock().unwrap();
    let mut queued = fixture.registration.clone();
    queued.env.insert(
        "XDG_STATE_HOME".into(),
        fixture
            ._dir
            .path()
            .join("retained-root")
            .to_string_lossy()
            .into_owned(),
    );
    assert!(
        herdr_idle_inhibitor::runtime::update::queue_if_pending(&fixture.paths, &queued)
            .await
            .unwrap()
    );
    drop(gate);
    let mut absent = fixture.paths.clone();
    absent.endpoint = fixture
        ._dir
        .path()
        .join("absent.sock")
        .to_string_lossy()
        .into_owned();
    assert!(
        herdr_idle_inhibitor::runtime::update::replay_pending(&absent)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(fixture.paths.runtime.join("pending"))
            .unwrap()
            .count(),
        1
    );
    herdr_idle_inhibitor::runtime::update::replay_pending(&fixture.paths)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_dir(fixture.paths.runtime.join("pending"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        status(&fixture.paths.endpoint, |_| true)
            .await
            .details
            .unwrap()
            .runtime
            .unwrap()
            .registrations
            .contains(&queued)
    );
}

#[tokio::test]
async fn unsupported_handoff_candidate_is_rejected_without_retiring_the_owner() {
    let mut fixture = Fixture::new().await;
    let original = std::fs::read(&fixture.paths.config).unwrap();
    let before = status(&fixture.paths.endpoint, |_| true).await;
    let unsupported = fixture._dir.path().join("pre-handoff-application");
    std::fs::write(&unsupported, "#!/bin/sh\ncase \"$1\" in\n--version) echo 'herdr-idle-inhibitor 0.1.0';;\n_capabilities) exit 77;;\n*) exit 77;;\nesac\n").unwrap();
    std::fs::set_permissions(&unsupported, std::fs::Permissions::from_mode(0o700)).unwrap();
    fixture.candidate = cache::stage(&unsupported, &fixture.paths.state).unwrap();
    let directory = fixture.worker(&fixture.plan(1000));
    marker(&directory.join("error.json")).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while herdr_idle_inhibitor::runtime::update::pending(&fixture.paths).unwrap() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let after = status(&fixture.paths.endpoint, |_| true).await;
    assert_eq!(
        after.status["monitor"]["instance_id"],
        before.status["monitor"]["instance_id"]
    );
    assert_eq!(after.status["control"]["paused"], true);
    assert_eq!(
        after.status["diagnostics"]["counters"]["native_acquires"],
        0
    );
    assert_eq!(std::fs::read(&fixture.paths.config).unwrap(), original);
    assert!(!directory.join("ready.json").exists());
    assert!(
        std::fs::read_to_string(directory.join("error.json"))
            .unwrap()
            .contains("does not support the automatic-update handoff protocol")
    );
}
