#[path = "support/tempdir.rs"]
mod fixtures;

use herdr_idle_inhibitor::{
    backend::{Kind, OwnedRequest, PowerBackend},
    herdr::{discovery::Registration, transport::read_frame},
    runtime::{
        ipc::{self, Operation},
        paths::Paths,
    },
};
use interprocess::local_socket::tokio::prelude::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::AsyncWriteExt;
#[derive(Clone)]
struct Fake {
    acquires: Arc<AtomicUsize>,
    releases: Arc<AtomicUsize>,
}
struct Held(Fake);
#[async_trait::async_trait]
impl PowerBackend for Fake {
    async fn acquire(&self, _: &Kind) -> anyhow::Result<Box<dyn OwnedRequest>> {
        self.acquires.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Held(self.clone())))
    }
}
#[async_trait::async_trait]
impl OwnedRequest for Held {
    async fn state(&mut self) -> anyhow::Result<String> {
        Ok("accepted".into())
    }
    async fn release(&mut self) -> anyhow::Result<()> {
        self.0.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
async fn fake_server(
    endpoint: String,
    working: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    root: String,
    registry: Arc<AtomicUsize>,
    capture: Option<Arc<serde_json::Value>>,
) {
    let listener = ipc::test_listener(&endpoint).unwrap();
    loop {
        let Ok(mut s) = listener.accept().await else {
            break;
        };
        let (working, enabled, root, registry, capture) = (
            working.clone(),
            enabled.clone(),
            root.clone(),
            registry.clone(),
            capture.clone(),
        );
        tokio::spawn(async move {
            let Some(frame) = read_frame(&mut tokio::io::BufReader::new(&mut s), 1024 * 1024)
                .await
                .unwrap()
            else {
                return;
            };
            let q: serde_json::Value = serde_json::from_slice(&frame).unwrap();
            let result = match q["method"].as_str().unwrap() {
                "ping" => serde_json::json!({"type":"pong","version":"0.9.3"}),
                "plugin.list" => {
                    registry.fetch_add(1, Ordering::SeqCst);
                    serde_json::json!({"type":"plugin_list","plugins":[{"plugin_id":"herdr-idle-inhibitor","plugin_root":root,"enabled":enabled.load(Ordering::SeqCst)}]})
                }
                "agent.list" => {
                    if let Some(capture) = capture {
                        let mut result = capture.as_ref().clone();
                        if !working.load(Ordering::SeqCst) {
                            for a in result["agents"].as_array_mut().unwrap() {
                                a["agent_status"] = serde_json::json!("done");
                            }
                        }
                        result
                    } else {
                        serde_json::json!({"type":"agent_list","agents":[{"terminal_id":"terminal","agent_status":if working.load(Ordering::SeqCst) {"working"} else {"idle"}}]})
                    }
                }
                _ => panic!("unexpected API"),
            };
            let mut b =
                serde_json::to_vec(&serde_json::json!({"id":q["id"],"result":result})).unwrap();
            b.push(b'\n');
            s.write_all(&b).await.unwrap();
        });
    }
}
async fn wait_status(
    endpoint: &str,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let outcome = match ipc::query(endpoint, Operation::GetStatus { details: false }).await {
            Ok(reply) if predicate(&reply.status) => return reply.status,
            Ok(reply) => serde_json::json!({"predicate_satisfied":false,"reply":reply}),
            Err(error) => serde_json::json!({"code":error.code,"exit":error.exit}),
        };
        assert!(
            tokio::time::Instant::now() < deadline,
            "status condition timeout after 8 seconds; endpoint={endpoint}; last outcome={outcome}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
#[tokio::test]
async fn real_transport_controller_replay_two_servers_pause_and_read_counters() {
    let dir = fixtures::tempdir();
    let root = dir.path().to_string_lossy().into_owned();
    let endpoint = |n: &str| {
        dir.path()
            .join(format!("{n}.sock"))
            .to_string_lossy()
            .into_owned()
    };
    let a = endpoint("a");
    let b = endpoint("b");
    let discovery = dir.path().join("sessions.json");
    std::fs::write(&discovery,serde_json::json!({"sessions":[{"name":"default","running":true,"socket_path":a},{"name":"named","running":true,"socket_path":b}]}).to_string()).unwrap();
    let bin = dir.path().join(if cfg!(windows) {
        "fake-herdr.exe"
    } else {
        "fake-herdr"
    });
    assert!(
        std::process::Command::new("rustc")
            .args(["tests/support/fake_cli.rs", "-o"])
            .arg(&bin)
            .status()
            .unwrap()
            .success()
    );
    let enabled = Arc::new(AtomicBool::new(true));
    let wa = Arc::new(AtomicBool::new(true));
    let wb = Arc::new(AtomicBool::new(true));
    let registry_a = Arc::new(AtomicUsize::new(0));
    let registry_b = Arc::new(AtomicUsize::new(0));
    let sa = tokio::spawn(fake_server(
        a.clone(),
        wa.clone(),
        enabled.clone(),
        root.clone(),
        registry_a,
        None,
    ));
    let sb = tokio::spawn(fake_server(
        b.clone(),
        wb.clone(),
        enabled.clone(),
        root,
        registry_b.clone(),
        None,
    ));
    let fake = Fake {
        acquires: Arc::new(AtomicUsize::new(0)),
        releases: Arc::new(AtomicUsize::new(0)),
    };
    let paths = Paths {
        config: dir.path().join("config.toml"),
        state: dir.path().join("logs"),
        runtime: dir.path().join("runtime"),
        endpoint: endpoint("control"),
    };
    let mut config = herdr_idle_inhibitor::runtime::config::Config {
        release_delay_secs: 0,
        ..Default::default()
    };
    config.linux.backend = "hypridle".into();
    std::fs::write(&paths.config, toml::to_string(&config).unwrap()).unwrap();
    let task = tokio::spawn(herdr_idle_inhibitor::controller::serve_with(
        paths.clone(),
        Box::new(fake.clone()),
    ));
    let r = Registration {
        endpoint: a.clone(),
        herdr_bin: bin,
        plugin_root: dir.path().into(),
        env: BTreeMap::from([(
            "HERDR_CONFIG_PATH".into(),
            discovery.to_string_lossy().into_owned(),
        )]),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
    };
    wait_status(&paths.endpoint, |_| true).await;
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh {
            registration: r.clone(),
        },
    )
    .await
    .unwrap();
    wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == 2
            && v["inhibition"]["resource_owned"] == true
    })
    .await;
    assert_eq!(fake.acquires.load(Ordering::SeqCst), 1);
    // An already-open older popup may still send this retired setting.
    for confirmation in [true, false] {
        // Send raw old-client JSON: the new SettingsPatch serializer omits this key.
        let reply = tokio::time::timeout(Duration::from_secs(2), async {
            let name = herdr_idle_inhibitor::herdr::transport::local_name(&paths.endpoint).unwrap();
            let mut stream = interprocess::local_socket::tokio::Stream::connect(name)
                .await
                .unwrap();
            herdr_idle_inhibitor::herdr::transport::check_peer(&stream).unwrap();
            let request_id = format!("legacy-confirmation-{confirmation}");
            let mut frame = serde_json::to_vec(&serde_json::json!({
                "protocol_version": 1,
                "request_id": request_id,
                "operation": {
                    "type": "ApplySettings",
                    "patch": {"hypridle_integration_confirmed": confirmation}
                }
            }))
            .unwrap();
            frame.push(b'\n');
            stream.write_all(&frame).await.unwrap();
            let bytes = read_frame(&mut tokio::io::BufReader::new(stream), 1024 * 1024)
                .await
                .unwrap()
                .unwrap();
            let reply: ipc::Reply = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(reply.protocol_version, 1);
            assert_eq!(reply.request_id, request_id);
            reply
        })
        .await
        .unwrap();
        assert!(reply.error.is_none());
        assert_eq!(reply.status["inhibition"]["resource_owned"], true);
        assert_eq!(fake.acquires.load(Ordering::SeqCst), 1);
        assert_eq!(fake.releases.load(Ordering::SeqCst), 0);
        assert!(
            !std::fs::read_to_string(&paths.config)
                .unwrap()
                .contains("hypridle_integration_confirmed")
        );
    }
    let before = ipc::query(&paths.endpoint, Operation::GetStatus { details: false })
        .await
        .unwrap()
        .status;
    for _ in 0..100 {
        ipc::query(&paths.endpoint, Operation::GetStatus { details: false })
            .await
            .unwrap();
    }
    let after = ipc::query(&paths.endpoint, Operation::GetStatus { details: false })
        .await
        .unwrap()
        .status;
    assert_eq!(
        after["diagnostics"]["counters"]["status_reads"]
            .as_u64()
            .unwrap()
            - before["diagnostics"]["counters"]["status_reads"]
                .as_u64()
                .unwrap(),
        101
    );
    for key in [
        "snapshot_hook",
        "snapshot_initial",
        "snapshot_resume",
        "native_acquires",
        "native_releases",
        "hints_received",
    ] {
        assert_eq!(
            before["diagnostics"]["counters"][key], after["diagnostics"]["counters"][key],
            "query changed {key}"
        );
    }
    println!(
        "{}",
        serde_json::json!({"experiment":"read_only_status","reads_before":before["diagnostics"]["counters"]["status_reads"],"reads_after":after["diagnostics"]["counters"]["status_reads"],"counters_before":before["diagnostics"]["counters"],"counters_after":after["diagnostics"]["counters"]})
    );
    wa.store(false, Ordering::SeqCst);
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh {
            registration: r.clone(),
        },
    )
    .await
    .unwrap();
    wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == 1
    })
    .await;
    assert_eq!(fake.releases.load(Ordering::SeqCst), 0);
    // Inject custom-endpoint handoff: discovery reports no standard servers and A exits.
    std::fs::write(&discovery, "{\"sessions\":[]}").unwrap();
    let mut registration_b = r.clone();
    registration_b.endpoint = b.clone();
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh {
            registration: registration_b,
        },
    )
    .await
    .unwrap();
    sa.abort();
    wait_status(&paths.endpoint, |v| {
        v["diagnostics"]["counters"]["discovery_attempts"]
            .as_u64()
            .unwrap()
            >= 2
            && registry_b.load(Ordering::SeqCst) > 0
            && v["inhibition"]["resource_owned"] == true
    })
    .await;
    ipc::query(&paths.endpoint, Operation::SetPaused { paused: true })
        .await
        .unwrap();
    let paused = wait_status(&paths.endpoint, |v| {
        v["inhibition"]["resource_owned"] == false
    })
    .await;
    assert_eq!(paused["observation"]["work"], "working");
    assert_eq!(fake.releases.load(Ordering::SeqCst), 1);
    ipc::query(&paths.endpoint, Operation::SetPaused { paused: false })
        .await
        .unwrap();
    wait_status(&paths.endpoint, |v| {
        v["inhibition"]["resource_owned"] == true
    })
    .await;
    assert_eq!(fake.acquires.load(Ordering::SeqCst), 2);
    wb.store(false, Ordering::SeqCst);
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh { registration: r },
    )
    .await
    .unwrap();
    wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == 0
            && v["inhibition"]["resource_owned"] == false
    })
    .await;
    assert_eq!(fake.releases.load(Ordering::SeqCst), 2);
    enabled.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(8), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    sa.abort();
    sb.abort();
}

#[tokio::test]
async fn minimized_live_capture_replays_through_controller_and_private_status() {
    let frame: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/herdr-0.9.3/agent-list.json")).unwrap();
    let captured = Arc::new(frame["result"].clone());
    let expected = captured["agents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["agent_status"] == "working")
        .count();
    assert!(expected > 0);
    let dir = fixtures::tempdir();
    let endpoint = |n: &str| {
        dir.path()
            .join(format!("{n}.sock"))
            .to_string_lossy()
            .into_owned()
    };
    let a = endpoint("capture");
    let discovery = dir.path().join("sessions.json");
    std::fs::write(
        &discovery,
        serde_json::json!({"sessions":[{"name":"redacted","running":true,"socket_path":a}]})
            .to_string(),
    )
    .unwrap();
    let bin = dir.path().join(if cfg!(windows) {
        "fake-herdr.exe"
    } else {
        "fake-herdr"
    });
    assert!(
        std::process::Command::new("rustc")
            .args(["tests/support/fake_cli.rs", "-o"])
            .arg(&bin)
            .status()
            .unwrap()
            .success()
    );
    let working = Arc::new(AtomicBool::new(true));
    let enabled = Arc::new(AtomicBool::new(true));
    let server = tokio::spawn(fake_server(
        a.clone(),
        working.clone(),
        enabled.clone(),
        dir.path().to_string_lossy().into_owned(),
        Default::default(),
        Some(captured),
    ));
    let fake = Fake {
        acquires: Default::default(),
        releases: Default::default(),
    };
    let paths = Paths {
        config: dir.path().join("config.toml"),
        state: dir.path().join("logs"),
        runtime: dir.path().join("runtime"),
        endpoint: endpoint("control"),
    };
    let mut config = herdr_idle_inhibitor::runtime::config::Config {
        release_delay_secs: 0,
        ..Default::default()
    };
    config.linux.backend = "hypridle".into();
    std::fs::write(&paths.config, toml::to_string(&config).unwrap()).unwrap();
    let owner = tokio::spawn(herdr_idle_inhibitor::controller::serve_with(
        paths.clone(),
        Box::new(fake.clone()),
    ));
    let registration = Registration {
        endpoint: a,
        herdr_bin: bin,
        plugin_root: dir.path().into(),
        env: BTreeMap::from([(
            "HERDR_CONFIG_PATH".into(),
            discovery.to_string_lossy().into_owned(),
        )]),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
    };
    wait_status(&paths.endpoint, |_| true).await;
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh {
            registration: registration.clone(),
        },
    )
    .await
    .unwrap();
    let status = wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == expected
            && v["inhibition"]["resource_owned"] == true
    })
    .await;
    herdr_idle_inhibitor::status::Status::from_wire(status.clone()).unwrap();
    assert_eq!(fake.acquires.load(Ordering::SeqCst), 1);
    assert!(
        status["diagnostics"]["counters"]["snapshot_valid"]
            .as_u64()
            .unwrap()
            > 0
    );
    // Recorded input above; completion below is an explicitly synthetic mutation.
    working.store(false, Ordering::SeqCst);
    ipc::query(
        &paths.endpoint,
        Operation::RegisterAndRefresh { registration },
    )
    .await
    .unwrap();
    let done = wait_status(&paths.endpoint, |v| {
        v["observation"]["work"] == "none" && v["inhibition"]["resource_owned"] == false
    })
    .await;
    assert_eq!(fake.releases.load(Ordering::SeqCst), 1);
    assert!(
        done["diagnostics"]["counters"]["snapshot_valid"]
            .as_u64()
            .unwrap()
            >= 2
    );
    println!(
        "{}",
        serde_json::json!({"experiment":"minimized_live_capture","working_agents_observed":expected,"initial_counters":status["diagnostics"]["counters"],"completion_counters":done["diagnostics"]["counters"],"native_acquires":fake.acquires.load(Ordering::SeqCst),"native_releases":fake.releases.load(Ordering::SeqCst)})
    );
    enabled.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(8), owner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.abort();
}

/// The handoff transfers registration context, never cached agent observations.
#[tokio::test]
async fn upgrade_releases_before_unlock_and_replays_all_roots_without_changing_settings() {
    use herdr_idle_inhibitor::runtime::update::Handoff;
    let dir = fixtures::tempdir();
    let endpoint = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
    let a = endpoint("upgrade-a.sock");
    let b = endpoint("upgrade-b.sock");
    let discovered = endpoint("upgrade-discovered.sock");
    let bin = dir
        .path()
        .join(if cfg!(windows) { "herdr.exe" } else { "herdr" });
    assert!(
        std::process::Command::new("rustc")
            .args(["tests/support/fake_cli.rs", "-o"])
            .arg(&bin)
            .status()
            .unwrap()
            .success()
    );
    let enabled = Arc::new(AtomicBool::new(true));
    let registry = Arc::new(AtomicUsize::new(0));
    let mut servers = vec![];
    let mut registrations = vec![];
    for (index, address) in [a.clone(), b.clone()].into_iter().enumerate() {
        let root = dir.path().join(format!("root-{index}"));
        std::fs::create_dir(&root).unwrap();
        let discovery = dir.path().join(format!("sessions-{index}.json"));
        let mut sessions =
            vec![serde_json::json!({"name":"default","running":true,"socket_path":address})];
        if index == 0 {
            sessions.push(
                serde_json::json!({"name":"discovered","running":true,"socket_path":discovered}),
            );
        }
        std::fs::write(
            &discovery,
            serde_json::json!({"sessions":sessions}).to_string(),
        )
        .unwrap();
        registrations.push(Registration {
            endpoint: address.clone(),
            herdr_bin: bin.clone(),
            plugin_root: root.clone(),
            env: BTreeMap::from([(
                "HERDR_CONFIG_PATH".into(),
                discovery.to_string_lossy().into_owned(),
            )]),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        });
        servers.push(tokio::spawn(fake_server(
            address,
            Arc::new(AtomicBool::new(true)),
            enabled.clone(),
            root.to_string_lossy().into_owned(),
            registry.clone(),
            None,
        )));
    }
    servers.push(tokio::spawn(fake_server(
        discovered.clone(),
        Arc::new(AtomicBool::new(true)),
        enabled.clone(),
        registrations[0].plugin_root.to_string_lossy().into_owned(),
        registry.clone(),
        None,
    )));
    let paths = Paths {
        config: dir.path().join("config.toml"),
        state: dir.path().join("logs"),
        runtime: dir.path().join("runtime"),
        endpoint: endpoint("upgrade-control.sock"),
    };
    let mut config = herdr_idle_inhibitor::runtime::config::Config {
        release_delay_secs: 0,
        ..Default::default()
    };
    config.linux.backend = "hypridle".into();
    std::fs::write(&paths.config, toml::to_string(&config).unwrap()).unwrap();
    let original = std::fs::read(&paths.config).unwrap();
    let fake = Fake {
        acquires: Arc::new(AtomicUsize::new(0)),
        releases: Arc::new(AtomicUsize::new(0)),
    };
    let old = tokio::spawn(herdr_idle_inhibitor::controller::serve_with(
        paths.clone(),
        Box::new(fake.clone()),
    ));
    wait_status(&paths.endpoint, |_| true).await;
    for registration in registrations.clone() {
        ipc::query(
            &paths.endpoint,
            Operation::RegisterAndRefresh { registration },
        )
        .await
        .unwrap();
    }
    let before = wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == 3
            && v["inhibition"]["resource_owned"] == true
    })
    .await;
    assert_eq!(fake.acquires.load(Ordering::SeqCst), 1);
    // Discovery for the non-installing root is no longer available, but its
    // previously discovered socket still responds and needs a fresh snapshot.
    std::fs::remove_file(&registrations[0].env["HERDR_CONFIG_PATH"]).unwrap();
    let reply = ipc::query(&paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
    let info = reply.details.unwrap().runtime.unwrap();
    assert_eq!(info.registrations.len(), 3);
    assert!(info.registrations.iter().any(|r| r.endpoint == discovered));
    tokio::time::timeout(Duration::from_secs(4), old)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        fake.releases.load(Ordering::SeqCst),
        1,
        "release must precede singleton availability"
    );
    let owner = herdr_idle_inhibitor::runtime::singleton::Owner::claim(paths.clone())
        .unwrap()
        .unwrap();
    drop(owner);
    assert_eq!(std::fs::read(&paths.config).unwrap(), original);
    let boot = Handoff {
        paths: paths.clone(),
        registrations: info.registrations,
        legacy_endpoints: vec![],
        effective_pause: None,
        source: None,
        rejected_updates: Default::default(),
    };
    let new = tokio::spawn(herdr_idle_inhibitor::controller::serve_with_handoff(
        boot,
        Box::new(fake.clone()),
    ));
    let after = wait_status(&paths.endpoint, |v| {
        v["observation"]["working_agents_observed"] == 3
            && v["inhibition"]["resource_owned"] == true
    })
    .await;
    assert_ne!(
        before["monitor"]["instance_id"],
        after["monitor"]["instance_id"]
    );
    assert_eq!(
        after["diagnostics"]["counters"]["snapshot_valid"], 3,
        "new owner must issue real snapshots, not reuse old observations"
    );
    assert_eq!(fake.acquires.load(Ordering::SeqCst), 2);
    assert_eq!(fake.releases.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(&paths.config).unwrap(), original);
    ipc::query(&paths.endpoint, Operation::PrepareUpgrade)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), new)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(fake.releases.load(Ordering::SeqCst), 2);
    for server in servers {
        server.abort();
    }
}
