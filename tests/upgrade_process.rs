//! Real cached executables and update helpers against captured Herdr traffic.
//! Linux overlay bytes produce different artifacts with the same package version.
#![cfg(target_os = "linux")]
#[path = "support/diagnostics.rs"]
mod diagnostics;
#[path = "support/tempdir.rs"]
mod fixtures;
use diagnostics::Diagnostics;
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
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::AsyncWriteExt;

struct Fixture {
    _dir: Diagnostics,
    paths: Paths,
    root: PathBuf,
    candidate: cache::Binary,
    old: cache::Binary,
    registration: Registration,
    snapshots: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self._dir.record(
                "captured_snapshot_calls",
                serde_json::json!(self.snapshots.load(Ordering::SeqCst)),
            );
            let _ = self
                ._dir
                .preserve("upgrade process fixture panicked; see original assertion");
        }
        self.server.abort();
    }
}
fn private_json(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
#[track_caller]
fn status<'a>(
    diagnostics: &'a mut Diagnostics,
    endpoint: &'a str,
    test: impl Fn(&serde_json::Value) -> bool + 'a,
) -> impl std::future::Future<Output = ipc::Reply> + 'a {
    let caller = std::panic::Location::caller();
    async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        diagnostics.record("wait", serde_json::json!({"kind":"status predicate", "endpoint":endpoint, "deadline_seconds":10,"caller":caller.to_string()}));
        let mut attempts = 0;
        loop {
            attempts += 1;
            match ipc::query(endpoint, Operation::GetStatus { details: true }).await {
                Ok(reply) => {
                    let matched = test(&reply.status);
                    diagnostics.record("status.last_reply", serde_json::json!({"status":reply.status,"details":reply.details,"error":reply.error,"predicate_satisfied":matched,"attempt":attempts}));
                    if matched { return reply; }
                }
                Err(error) => diagnostics.record("status.last_error", serde_json::json!({"endpoint":endpoint,"code":error.code,"exit":error.exit,"attempt":attempts})),
            }
            diagnostics.check(
                tokio::time::Instant::now() < deadline,
                "status replay deadline expired after 10 seconds",
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}
async fn marker(diagnostics: &mut Diagnostics, path: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    diagnostics.record(
        "wait",
        serde_json::json!({"kind":"update marker", "path":path,"deadline_seconds":10}),
    );
    while !path.is_file() {
        diagnostics.check(
            tokio::time::Instant::now() < deadline,
            &format!(
                "update marker deadline after 10 seconds: {}",
                path.display()
            ),
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn owner_stopped(diagnostics: &mut Diagnostics, endpoint: &str, timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    diagnostics.record("wait", serde_json::json!({"kind":"owner shutdown", "endpoint":endpoint,"deadline_seconds":timeout.as_secs()}));
    loop {
        match ipc::query(endpoint, Operation::GetStatus { details: false }).await {
            Ok(reply) => diagnostics.record("status.last_reply", serde_json::json!({"status":reply.status,"details":reply.details,"error":reply.error,"predicate_satisfied":false})),
            Err(error) => {
                diagnostics.record("status.last_error", serde_json::json!({"endpoint":endpoint,"code":error.code,"exit":error.exit}));
                return;
            }
        }
        diagnostics.check(
            tokio::time::Instant::now() < deadline,
            "owner shutdown deadline expired",
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn gate_released(diagnostics: &mut Diagnostics, paths: &Paths, timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    diagnostics.record("wait", serde_json::json!({"kind":"update gate release", "path":paths.runtime.join("update.lock"),"deadline_seconds":timeout.as_secs()}));
    loop {
        let pending = herdr_idle_inhibitor::runtime::update::pending(paths).unwrap();
        diagnostics.record("gate.last_pending", serde_json::json!(pending));
        if !pending {
            return;
        }
        diagnostics.check(
            tokio::time::Instant::now() < deadline,
            "update gate release deadline expired",
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
impl Fixture {
    async fn new() -> Self {
        let mut dir = Diagnostics::new(fixtures::tempdir(), None);
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
        dir.record(
            "images",
            serde_json::json!({"old":old,"candidate":candidate,"installed":executable}),
        );
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
        dir.spawn(
            "monitor",
            Command::new(&old.path)
                .env("TOKIO_WORKER_THREADS", "2")
                .arg("_resume")
                .arg(&boot_path),
        )
        .unwrap();
        status(&mut dir, &paths.endpoint, |v| {
            v["observation"]["work"] == "working"
        })
        .await;
        Self {
            _dir: dir,
            paths,
            root,
            candidate,
            old,
            registration,
            snapshots,
            server,
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
        let count = self._dir.children_mut().len();
        let directory = self._dir.path().join(format!("worker-{count}"));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("plan.json");
        private_json(&path, plan);
        self._dir.record("update_plan", serde_json::json!(plan));
        self._dir
            .spawn(
                "upgrade-worker",
                Command::new(&self.old.path)
                    .env("TOKIO_WORKER_THREADS", "2")
                    .arg("_upgrade_worker")
                    .arg(path),
            )
            .unwrap();
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
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    let instance = before.status["monitor"]["instance_id"].clone();
    let directory = fixture.worker(&fixture.plan(5000));
    marker(&mut fixture._dir, &directory.join("ready.json")).await;
    assert_eq!(
        status(&mut fixture._dir, &fixture.paths.endpoint, |_| true)
            .await
            .status["monitor"]["instance_id"],
        instance
    );
    fixture.publish("old-commit");
    assert_eq!(
        status(&mut fixture._dir, &fixture.paths.endpoint, |_| true)
            .await
            .status["monitor"]["instance_id"],
        instance,
        "file replacement alone cannot terminate the working monitor"
    );
    let snapshots = fixture.snapshots.load(Ordering::SeqCst);
    fixture.publish("new-commit");
    marker(&mut fixture._dir, &directory.join("complete.json")).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    let directory = fixture.worker(&fixture.plan(1000));
    marker(&mut fixture._dir, &directory.join("ready.json")).await;
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
    marker(&mut fixture._dir, &directory.join("error.json")).await;
    gate_released(&mut fixture._dir, &fixture.paths, Duration::from_secs(10)).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
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
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    let mut plan = fixture.plan(5000);
    plan.candidate = fixture.old.clone();
    plan.publication.digest = fixture.old.digest.clone();
    let directory = fixture.worker(&plan);
    marker(&mut fixture._dir, &directory.join("ready.json")).await;
    private_json(
        &PathBuf::from(format!(
            "{}.plugins",
            fixture.registration.env["HERDR_CONFIG_PATH"]
        )),
        &serde_json::json!({"result":{"plugins":[{"plugin_id":"herdr-idle-inhibitor","enabled":true,"plugin_root":fixture.root,"source":{"kind":"github","resolved_commit":"new-commit"}}]}}),
    );
    marker(&mut fixture._dir, &directory.join("complete.json")).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
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
    marker(&mut fixture._dir, &directory.join("ready.json")).await;
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
    marker(&mut fixture._dir, &directory.join("complete.json")).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
    let mut fixture = Fixture::new().await;
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
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
        status(&mut fixture._dir, &fixture.paths.endpoint, |_| true)
            .await
            .status["monitor"]["instance_id"],
        before.status["monitor"]["instance_id"]
    );
    fixture.publish("new-commit");
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
    let mut fixture = Fixture::new().await;
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
    owner_stopped(
        &mut fixture._dir,
        &fixture.paths.endpoint,
        Duration::from_secs(5),
    )
    .await;
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
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
    marker(&mut fixture._dir, &directory.join("ready.json")).await;
    fixture.publish("new-commit");
    marker(&mut fixture._dir, &directory.join("error.json")).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
        status(&mut fixture._dir, &fixture.paths.endpoint, |_| true)
            .await
            .status["monitor"]["instance_id"],
        instance,
        "recovery must not repeatedly retry the same failed artifact"
    );
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
}

#[tokio::test]
async fn resident_monitor_detects_same_version_file_replacement_without_herdr_events() {
    let mut fixture = Fixture::new().await;
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    let config = std::fs::read(&fixture.paths.config).unwrap();
    fixture.publish("new-commit"); // No startup, event hook or prepare-worker call.
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |v| {
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
    marker(&mut fixture._dir, &entered).await;
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
    marker(&mut fixture._dir, &directory.join("error.json")).await;
    gate_released(&mut fixture._dir, &fixture.paths, Duration::from_secs(10)).await;
    let reply = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
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
    let mut fixture = Fixture::new().await;
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
        status(&mut fixture._dir, &fixture.paths.endpoint, |_| true)
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
    let before = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    let unsupported = fixture._dir.path().join("pre-handoff-application");
    std::fs::write(&unsupported, "#!/bin/sh\ncase \"$1\" in\n--version) echo 'herdr-idle-inhibitor 0.1.0';;\n_capabilities) exit 77;;\n*) exit 77;;\nesac\n").unwrap();
    std::fs::set_permissions(&unsupported, std::fs::Permissions::from_mode(0o700)).unwrap();
    fixture.candidate = cache::stage(&unsupported, &fixture.paths.state).unwrap();
    let directory = fixture.worker(&fixture.plan(1000));
    marker(&mut fixture._dir, &directory.join("error.json")).await;
    gate_released(&mut fixture._dir, &fixture.paths, Duration::from_secs(10)).await;
    let after = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
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

#[tokio::test]
async fn shutdown_timeout_diagnostics_replace_startup_context_and_keep_fresh_reply() {
    use futures_util::FutureExt;
    let artifacts = fixtures::tempdir();
    let mut diagnostics = Diagnostics::new(fixtures::tempdir(), Some(artifacts.path().to_owned()));
    let endpoint = diagnostics
        .path()
        .join("reply.sock")
        .to_string_lossy()
        .into_owned();
    diagnostics.record(
        "wait",
        serde_json::json!({"kind":"old startup", "deadline_seconds":10}),
    );
    diagnostics.record(
        "status.last_reply",
        serde_json::json!({"stale_startup":true}),
    );
    let listener = ipc::test_listener(&endpoint).unwrap();
    let peer = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        let frame = herdr_idle_inhibitor::herdr::transport::read_frame(&mut stream, 1024 * 1024)
            .await
            .unwrap()
            .unwrap();
        let request: serde_json::Value = serde_json::from_slice(&frame).unwrap();
        let status = herdr_idle_inhibitor::status::Status::unavailable(
            "fixture_reply",
            "fresh shutdown-loop reply",
        );
        let mut bytes = serde_json::to_vec(&serde_json::json!({"protocol_version":1,"request_id":request["request_id"],"status":status,"details":null,"error":null})).unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).await.unwrap();
    });
    let result =
        std::panic::AssertUnwindSafe(owner_stopped(&mut diagnostics, &endpoint, Duration::ZERO))
            .catch_unwind()
            .await;
    assert!(result.is_err());
    peer.await.unwrap();
    let directory = std::fs::read_dir(artifacts.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("report.json")).unwrap()).unwrap();
    assert_eq!(data["context"]["wait"]["kind"], "owner shutdown");
    assert_eq!(data["context"]["wait"]["deadline_seconds"], 0);
    assert_eq!(
        data["context"]["status.last_reply"]["status"]["error"]["message"],
        "fresh shutdown-loop reply"
    );
    assert_eq!(
        data["context"]["status.last_reply"]["predicate_satisfied"],
        false
    );
}

#[tokio::test]
async fn gate_timeout_diagnostics_replace_completed_marker_context_with_locked_gate() {
    use futures_util::FutureExt;
    let artifacts = fixtures::tempdir();
    let mut diagnostics = Diagnostics::new(fixtures::tempdir(), Some(artifacts.path().to_owned()));
    let runtime = diagnostics.path().join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let gate_path = runtime.join("update.lock");
    let gate = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&gate_path)
        .unwrap();
    std::fs::set_permissions(&gate_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    gate.try_lock().unwrap();
    let paths = Paths {
        config: diagnostics.path().join("config.toml"),
        state: diagnostics.path().join("state"),
        runtime,
        endpoint: "unused".into(),
    };
    diagnostics.record("wait", serde_json::json!({"kind":"completed marker"}));
    diagnostics.record(
        "status.last_error",
        serde_json::json!({"code":"historical startup error"}),
    );
    let result =
        std::panic::AssertUnwindSafe(gate_released(&mut diagnostics, &paths, Duration::ZERO))
            .catch_unwind()
            .await;
    assert!(result.is_err());
    let directory = std::fs::read_dir(artifacts.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("report.json")).unwrap()).unwrap();
    assert_eq!(data["context"]["wait"]["kind"], "update gate release");
    assert_eq!(
        data["context"]["wait"]["path"],
        gate_path.to_string_lossy().as_ref()
    );
    assert_eq!(data["context"]["gate.last_pending"], true);
    assert!(
        data["context_sequence"]["status.last_error"]
            .as_u64()
            .unwrap()
            < data["context_sequence"]["wait"].as_u64().unwrap()
    );
    assert!(
        data["context_sequence"]["wait"].as_u64().unwrap()
            < data["context_sequence"]["gate.last_pending"]
                .as_u64()
                .unwrap()
    );
}

#[tokio::test]
async fn monitor_without_handover_metadata_is_rejected_without_changing_registration() {
    let mut fixture = Fixture::new().await;
    let mut reply = status(&mut fixture._dir, &fixture.paths.endpoint, |_| true).await;
    reply.details.as_mut().unwrap().runtime = None;
    let original_config = std::fs::read(&fixture.paths.config).unwrap();
    let registry = PathBuf::from(format!(
        "{}.plugins",
        fixture.registration.env["HERDR_CONFIG_PATH"]
    ));
    let original_registry = std::fs::read(&registry).unwrap();
    ipc::query(&fixture.paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
    owner_stopped(
        &mut fixture._dir,
        &fixture.paths.endpoint,
        Duration::from_secs(5),
    )
    .await;
    let listener = ipc::test_listener(&fixture.paths.endpoint).unwrap();
    let peer = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap();
        let frame = herdr_idle_inhibitor::herdr::transport::read_frame(&mut stream, 1024 * 1024)
            .await
            .unwrap()
            .unwrap();
        let request: ipc::Request = serde_json::from_slice(&frame).unwrap();
        assert!(matches!(
            request.operation,
            Operation::GetStatus { details: true }
        ));
        reply.request_id = request.request_id;
        let mut bytes = serde_json::to_vec(&reply).unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).await.unwrap();
    });
    let directory = fixture.worker(&fixture.plan(1000));
    marker(&mut fixture._dir, &directory.join("error.json")).await;
    peer.await.unwrap();
    let error: String =
        serde_json::from_slice(&std::fs::read(directory.join("error.json")).unwrap()).unwrap();
    assert!(
        error.contains("monitor does not support automatic-update handover"),
        "{error}"
    );
    assert!(!directory.join("ready.json").exists());
    assert!(!directory.join("complete.json").exists());
    assert_eq!(
        std::fs::read(&fixture.paths.config).unwrap(),
        original_config
    );
    assert_eq!(std::fs::read(registry).unwrap(), original_registry);
}
