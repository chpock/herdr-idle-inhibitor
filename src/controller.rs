use crate::{
    backend::{self, Kind, NativeBackend, OwnedRequest, PowerBackend},
    clock,
    diagnostics::Logger,
    herdr::{
        discovery::{Registration, Session},
        protocol, transport,
    },
    model::*,
    policy::{Decision, Policy},
    runtime::{
        config::ConfigStore,
        ipc::{self, Details, Incoming, Operation, Reply, ServerRow},
        paths::Paths,
        singleton::Owner,
    },
    status::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::sync::mpsc;

struct Root {
    registration: Registration,
    generation: u64,
    busy: bool,
    due: u64,
    healthy: bool,
    last_check: Option<u64>,
    eligibility_checked: Option<u64>,
    enabled: Option<bool>,
    last_enabled: Option<u64>,
    endpoints: BTreeSet<String>,
    registered: BTreeSet<String>,
}
struct Server {
    observation: ServerObservation,
    generation: u64,
    request: u64,
    busy: bool,
    dirty: bool,
    due: u64,
    trigger: &'static str,
    errors: u32,
    version: Option<String>,
    transport_absent: bool,
    absent_since: Option<u64>,
}
impl Server {
    fn new(now: u64) -> Self {
        Self {
            observation: ServerObservation::default(),
            generation: 0,
            request: 0,
            busy: false,
            dirty: false,
            due: now,
            trigger: "initial",
            errors: 0,
            version: None,
            transport_absent: false,
            absent_since: None,
        }
    }
}
enum ResultEvent {
    Root {
        key: String,
        generation: u64,
        discovery: anyhow::Result<Vec<Session>>,
        eligibility: anyhow::Result<bool>,
    },
    Snapshot {
        endpoint: String,
        generation: u64,
        request: u64,
        result: anyhow::Result<(Vec<Agent>, String)>,
    },
    Native {
        generation: u64,
        owned: bool,
        state: String,
        failed: bool,
        acquired: bool,
        released: bool,
        lost: bool,
        issue: Option<Issue>,
    },
}
enum NativeCommand {
    Acquire {
        generation: u64,
        kind: Kind,
    },
    Release {
        generation: u64,
    },
    Check {
        generation: u64,
    },
    Stop {
        done: tokio::sync::oneshot::Sender<()>,
    },
}
fn native_worker(
    tx: mpsc::Sender<ResultEvent>,
    backend: Box<dyn PowerBackend>,
) -> mpsc::Sender<NativeCommand> {
    let (commands, mut rx) = mpsc::channel(2);
    tokio::spawn(async move {
        let mut resource: Option<Box<dyn OwnedRequest>> = None;
        while let Some(command) = rx.recv().await {
            let (generation, mut acquired, mut released, mut lost, mut failed) = (
                match command {
                    NativeCommand::Acquire { generation, .. }
                    | NativeCommand::Release { generation }
                    | NativeCommand::Check { generation } => generation,
                    NativeCommand::Stop { .. } => 0,
                },
                false,
                false,
                false,
                false,
            );
            let mut issue = None;
            match command {
                NativeCommand::Acquire { kind, .. } => {
                    if resource.is_none() {
                        match backend.probe(&kind).await {
                            Err(i) => {
                                failed = true;
                                issue = Some(i);
                            }
                            Ok(()) => match backend.acquire(&kind).await {
                                Ok(r) => {
                                    resource = Some(r);
                                    acquired = true;
                                }
                                Err(_) => failed = true,
                            },
                        }
                    }
                }
                NativeCommand::Release { .. } => {
                    if let Some(r) = resource.as_mut() {
                        failed = r.release().await.is_err();
                        if !failed {
                            resource = None;
                            released = true;
                        }
                    }
                }
                NativeCommand::Check { .. } => (),
                NativeCommand::Stop { done } => {
                    if let Some(mut r) = resource.take() {
                        let _ = r.release().await;
                    }
                    let _ = done.send(());
                    break;
                }
            }
            let state = if let Some(r) = &mut resource {
                match r.state().await {
                    Ok(s) => s,
                    Err(_) => {
                        failed = true;
                        lost = true;
                        if r.release().await.is_ok() {
                            released = true;
                            "none".into()
                        } else {
                            "unknown".into()
                        }
                    }
                }
            } else if failed {
                "failed".into()
            } else {
                "none".into()
            };
            if state == "none" && resource.is_some() {
                resource = None;
                released = true;
            }
            let _ = tx
                .send(ResultEvent::Native {
                    generation,
                    owned: resource.is_some(),
                    state,
                    failed,
                    acquired,
                    released,
                    lost,
                    issue,
                })
                .await;
        }
        if let Some(mut r) = resource {
            let _ = r.release().await;
        }
    });
    commands
}
struct Native {
    owned: bool,
    backend: Option<String>,
    request_state: String,
    last_error: Option<Issue>,
}
impl Default for Native {
    fn default() -> Self {
        Self {
            owned: false,
            backend: None,
            request_state: "none".into(),
            last_error: None,
        }
    }
}
pub struct Controller {
    paths: Paths,
    config: ConfigStore,
    roots: BTreeMap<String, Root>,
    servers: BTreeMap<String, Server>,
    policy: Policy,
    decision: Decision,
    counters: Counters,
    native: Native,
    selected: Option<Kind>,
    native_generation: u64,
    native_busy: bool,
    native_due: u64,
    native_errors: u32,
    native_obsolete: bool,
    suspended: bool,
    started: u64,
    last_tick: u64,
    revalidation_until: u64,
    last_useful: u64,
    gone_since: Option<u64>,
    scope_established: bool,
    instance: String,
    seq: u64,
    evaluated_at: u64,
    desktop: String,
    bus: Option<String>,
    conflict: bool,
    logger: Logger,
    snapshot_inflight: usize,
    root_inflight: usize,
    snapshot_jobs: BTreeMap<(String, u64, u64), tokio::task::AbortHandle>,
    root_jobs: BTreeMap<(String, u64), tokio::task::AbortHandle>,
}
impl Controller {
    fn new(paths: Paths) -> anyhow::Result<Self> {
        let now = clock::now_ms();
        let config = ConfigStore::open(paths.config.clone());
        let logger = Logger::new(&paths.state)?;
        Ok(Self {
            paths,
            config,
            roots: BTreeMap::new(),
            servers: BTreeMap::new(),
            policy: Policy::default(),
            decision: Decision::off("unknown"),
            counters: Counters::default(),
            native: Native::default(),
            selected: None,
            native_generation: 0,
            native_busy: false,
            native_due: now,
            native_errors: 0,
            native_obsolete: false,
            suspended: false,
            started: now,
            last_tick: now,
            revalidation_until: now + 5000,
            last_useful: now,
            gone_since: None,
            scope_established: false,
            instance: uuid::Uuid::new_v4().to_string(),
            seq: 0,
            evaluated_at: clock::unix_ms(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
            conflict: false,
            logger,
            snapshot_inflight: 0,
            root_inflight: 0,
            snapshot_jobs: BTreeMap::new(),
            root_jobs: BTreeMap::new(),
        })
    }
    fn coverage(&self, now: u64) -> bool {
        self.scope_established
            && self.roots.values().all(|r| {
                r.healthy
                    && r.last_check
                        .is_some_and(|t| now.saturating_sub(t) < CHECK_FRESH_MS)
            })
    }
    fn observation(&self, now: u64) -> Observation {
        aggregate(
            self.servers.values().map(|s| &s.observation),
            self.coverage(now),
            now,
        )
    }
    fn eligibility(&self, now: u64) -> (bool, bool) {
        let fresh = self.roots.values().any(|r| {
            r.enabled == Some(true)
                && r.eligibility_checked
                    .is_some_and(|t| now.saturating_sub(t) < CHECK_FRESH_MS)
        });
        let retained = self.roots.values().any(|r| {
            r.enabled != Some(false)
                && r.last_enabled
                    .is_some_and(|t| now < t.saturating_add(RETAIN_MS))
        });
        (fresh, retained)
    }
    fn status(&self, now: u64) -> Status {
        let mut issues = vec![];
        if !self.config.valid {
            issues.push(Issue::new(
                "invalid_config",
                "Correct the configuration and use Reload",
            ));
        }
        if !self.config.pause_persisted {
            issues.push(Issue::new(
                "pause_not_persisted",
                "Pause is active but could not be saved; restart may restore the previous setting",
            ));
        }
        if self.config.error.is_some() && self.config.valid && self.config.pause_persisted {
            issues.push(Issue::new(
                "config_write_or_reload_failed",
                "Configuration operation failed; inspect Settings and reload external edits",
            ));
        }
        if self.conflict {
            issues.push(Issue::new("desktop_context_conflict","Registered desktop contexts conflict; only one active desktop session is supported"));
        }
        if !self.coverage(now) {
            issues.push(Issue::new(
                "discovery_failed",
                "Discovery is incomplete, failed, or stale",
            ));
        }
        let observation = self.observation(now);
        if observation.servers_unknown.is_some_and(|n| n > 0) {
            issues.push(Issue::new(
                "snapshot_stale",
                "Some endpoints have no current valid observation",
            ));
        }
        if let Some(i) = &self.native.last_error {
            issues.push(i.clone());
        }
        Status {
            schema_version: 1,
            available: true,
            error: None,
            monitor: Some(Monitor {
                instance_id: self.instance.clone(),
                pid: std::process::id(),
                app_version: env!("CARGO_PKG_VERSION").into(),
                uptime_ms: now.saturating_sub(self.started),
                snapshot_seq: self.seq,
                evaluated_at_unix_ms: self.evaluated_at,
            }),
            control: Some(Control {
                paused: self.config.config.paused,
                pause_persisted: self.config.pause_persisted,
            }),
            observation,
            inhibition: Inhibition {
                desired: Some(self.decision.desired),
                reason: self.decision.reason.clone(),
                backend: self.native.backend.clone(),
                resource_owned: if self.native_busy && !self.native.owned {
                    None
                } else {
                    Some(self.native.owned)
                },
                request_state: self.native.request_state.clone(),
                release_in_ms: self.decision.release_in_ms,
                retention_remaining_ms: self.decision.retention_remaining_ms,
                last_error: self.native.last_error.clone(),
            },
            diagnostics: Some(Diagnostics {
                issues,
                known_limitations: if cfg!(windows) {
                    vec!["windows_modern_standby_battery".into()]
                } else if self.selected == Some(Kind::Kde) {
                    vec!["kde_powerdevil_suppressed_owner_cleanup".into()]
                } else {
                    vec![]
                },
                counters: self.counters.clone(),
            }),
        }
    }
    fn register(&mut self, mut r: Registration, now: u64) -> anyhow::Result<()> {
        r.validate()?;
        r.endpoint = crate::herdr::discovery::normalize_endpoint(&r.endpoint)?;
        anyhow::ensure!(
            self.roots.len() < 128 || self.roots.contains_key(&r.root_key()),
            "too many roots"
        );
        if !self.roots.is_empty() && (self.desktop != r.desktop || self.bus != r.bus) {
            self.conflict = true;
            anyhow::bail!("conflicting desktop session context");
        }
        if self.roots.is_empty() {
            anyhow::ensure!(
                self.bus == r.bus,
                "bootstrap desktop bus differs from owner"
            );
            self.desktop = r.desktop.clone();
        }
        let key = r.root_key();
        let r_key = key.clone();
        let endpoint = r.endpoint.clone();
        if !self.roots.contains_key(&key) {
            self.scope_established = false;
            self.roots.insert(
                key,
                Root {
                    registration: r,
                    generation: 0,
                    busy: false,
                    due: now,
                    healthy: false,
                    last_check: None,
                    eligibility_checked: None,
                    enabled: None,
                    last_enabled: None,
                    endpoints: BTreeSet::new(),
                    registered: BTreeSet::from([endpoint.clone()]),
                },
            );
        }
        if let Some(root) = self.roots.get_mut(&r_key) {
            root.registered.insert(endpoint.clone());
        }
        self.servers
            .entry(endpoint.clone())
            .or_insert_with(|| Server::new(now));
        self.counters.hints_received += 1;
        for s in self.servers.values_mut() {
            if s.busy || s.trigger == "hook" {
                s.dirty |= s.busy;
                self.counters.hints_coalesced += 1;
            } else {
                s.due = s.due.min(now + 100);
                s.trigger = "hook";
            }
        }
        Ok(())
    }
    fn control(&mut self, incoming: Incoming, now: u64, native: &mpsc::Sender<NativeCommand>) {
        let mut details = false;
        let readonly = matches!(incoming.request.operation, Operation::GetStatus { .. });
        if !readonly {
            self.observe_gap(now);
        }
        let result = match incoming.request.operation {
            Operation::GetStatus { details: d } => {
                self.counters.status_reads += 1;
                details = d;
                Ok(())
            }
            Operation::RegisterAndRefresh { registration } => self.register(registration, now),
            Operation::SetPaused { paused } => self.config.set_paused(paused),
            Operation::ApplySettings { patch } => {
                let mut c = self.config.config.clone();
                if let Some(v) = patch.release_delay_secs {
                    c.release_delay_secs = v;
                }
                if let Some(v) = patch.linux_backend {
                    c.linux.backend = v;
                }
                if let Some(v) = patch.hypridle_integration_confirmed {
                    c.linux.hypridle_integration_confirmed = v;
                }
                self.config.apply(c)
            }
            Operation::ReloadSettings => self.config.reload(),
        };
        if !readonly {
            self.evaluate(now, native);
        }
        let error = result.err().map(|_| {
            Issue::new(
                "control_failed",
                "Operation rejected; check configuration, external edits, and desktop context",
            )
        });
        let details = details.then(|| Details {
            config: self.config.config.clone(),
            config_path: self.paths.config.to_string_lossy().into_owned(),
            servers: self
                .servers
                .iter()
                .map(|(e, s)| ServerRow {
                    endpoint: e.clone(),
                    version: s.version.clone(),
                    current: s.observation.current(now),
                    age_ms: s.observation.snapshot_at.map(|t| now.saturating_sub(t)),
                })
                .collect(),
        });
        let _ = incoming.reply.send(Reply {
            protocol_version: 1,
            request_id: incoming.request.request_id,
            status: serde_json::to_value(self.status(now)).unwrap(),
            error,
            details,
        });
    }
    fn schedule(&mut self, now: u64, tx: &mpsc::Sender<ResultEvent>) {
        for endpoint in &self.config.config.additional_endpoints {
            let endpoint = crate::herdr::discovery::normalize_endpoint(endpoint)
                .unwrap_or_else(|_| endpoint.clone());
            self.servers
                .entry(endpoint)
                .or_insert_with(|| Server::new(now));
        }
        for (key, r) in &mut self.roots {
            if r.busy || r.due > now || self.root_inflight >= 8 {
                continue;
            }
            r.busy = true;
            r.due = now + 5000;
            self.root_inflight += 1;
            self.counters.discovery_attempts += 1;
            let (key, generation, registration, tx) = (
                key.clone(),
                r.generation,
                r.registration.clone(),
                tx.clone(),
            );
            let mut registered: Vec<_> = r.registered.iter().cloned().collect();
            registered.sort_by_key(|e| {
                !self
                    .servers
                    .get(e)
                    .is_some_and(|s| s.observation.current(now))
            });
            let job_key = (key.clone(), generation);
            let job = tokio::spawn(async move {
                let discovery = crate::herdr::discovery::discover(&registration).await;
                let eligibility = async {
                    let mut candidates = registered;
                    for session in discovery
                        .as_ref()
                        .ok()
                        .into_iter()
                        .flatten()
                        .filter(|s| s.running)
                    {
                        if !candidates.contains(&session.socket_path) {
                            candidates.push(session.socket_path.clone());
                        }
                    }
                    for endpoint in candidates.into_iter().take(8) {
                        let id = uuid::Uuid::new_v4().to_string();
                        if let Ok(frame) = transport::call(
                            &endpoint,
                            &id,
                            "plugin.list",
                            serde_json::json!({"plugin_id":crate::herdr::discovery::PLUGIN_ID}),
                        )
                        .await
                            && let Ok(plugins) = protocol::parse_plugins(&frame, &id)
                        {
                            return Ok(plugins.iter().any(|p| {
                                p.plugin_id == crate::herdr::discovery::PLUGIN_ID
                                    && p.enabled
                                    && std::path::Path::new(&p.plugin_root)
                                        == registration.plugin_root
                            }));
                        }
                    }
                    Err(anyhow::anyhow!("no installation endpoint answered"))
                }
                .await;
                let _ = tx
                    .send(ResultEvent::Root {
                        key,
                        generation,
                        discovery,
                        eligibility,
                    })
                    .await;
            });
            self.root_jobs.insert(job_key, job.abort_handle());
        }
        for (endpoint, s) in &mut self.servers {
            if s.busy || s.due > now || self.snapshot_inflight >= 8 {
                continue;
            }
            s.busy = true;
            s.request += 1;
            self.snapshot_inflight += 1;
            match s.trigger {
                "initial" => self.counters.snapshot_initial += 1,
                "hook" => self.counters.snapshot_hook += 1,
                "resume" => self.counters.snapshot_resume += 1,
                _ => self.counters.snapshot_poll += 1,
            }
            let (endpoint, generation, request, tx) =
                (endpoint.clone(), s.generation, s.request, tx.clone());
            let version = s.version.clone();
            let job_key = (endpoint.clone(), generation, request);
            let job = tokio::spawn(async move {
                let result = async {
                    let version = if let Some(v) = version {
                        v
                    } else {
                        let id = uuid::Uuid::new_v4().to_string();
                        let b =
                            transport::call(&endpoint, &id, "ping", serde_json::json!({})).await?;
                        protocol::parse_ping(&b, &id)?
                    };
                    let id = uuid::Uuid::new_v4().to_string();
                    let bytes =
                        transport::call(&endpoint, &id, "agent.list", serde_json::json!({}))
                            .await?;
                    Ok((protocol::parse_agents(&bytes, &id)?, version))
                }
                .await;
                let _ = tx
                    .send(ResultEvent::Snapshot {
                        endpoint,
                        generation,
                        request,
                        result,
                    })
                    .await;
            });
            self.snapshot_jobs.insert(job_key, job.abort_handle());
        }
    }
    fn result(&mut self, result: ResultEvent, now: u64) {
        self.observe_gap(now);
        match result {
            ResultEvent::Root {
                key,
                generation,
                discovery,
                eligibility,
            } => {
                if self.root_jobs.remove(&(key.clone(), generation)).is_some() {
                    self.root_inflight = self.root_inflight.saturating_sub(1);
                }
                let Some(r) = self.roots.get_mut(&key) else {
                    return;
                };
                if r.generation != generation {
                    self.counters.old_results_ignored += 1;
                    return;
                }
                r.busy = false;
                match discovery {
                    Ok(sessions) => {
                        r.healthy = true;
                        r.last_check = Some(now);
                        r.endpoints = sessions
                            .into_iter()
                            .filter(|s| s.running)
                            .map(|s| s.socket_path)
                            .collect();
                        self.last_useful = now;
                        for e in &r.endpoints {
                            self.servers
                                .entry(e.clone())
                                .or_insert_with(|| Server::new(now));
                        }
                    }
                    Err(_) => {
                        r.healthy = false;
                        self.counters.discovery_failures += 1;
                    }
                }
                match eligibility {
                    Ok(enabled) => {
                        r.enabled = Some(enabled);
                        r.eligibility_checked = Some(now);
                        if enabled {
                            r.last_enabled = Some(now);
                        }
                    }
                    Err(_) => {
                        r.enabled = None;
                    }
                }
                self.scope_established = self.roots.values().all(|r| r.last_check.is_some());
            }
            ResultEvent::Snapshot {
                endpoint,
                generation,
                request,
                result,
            } => {
                if self
                    .snapshot_jobs
                    .remove(&(endpoint.clone(), generation, request))
                    .is_some()
                {
                    self.snapshot_inflight = self.snapshot_inflight.saturating_sub(1);
                }
                let Some(s) = self.servers.get_mut(&endpoint) else {
                    self.counters.old_results_ignored += 1;
                    return;
                };
                if s.generation != generation || s.request != request {
                    self.counters.old_results_ignored += 1;
                    return;
                }
                s.busy = false;
                s.transport_absent = result.as_ref().err().is_some_and(|e| {
                    e.chain()
                        .filter_map(|e| e.downcast_ref::<std::io::Error>())
                        .any(|e| {
                            matches!(
                                e.kind(),
                                std::io::ErrorKind::NotFound
                                    | std::io::ErrorKind::ConnectionRefused
                            )
                        })
                });
                let valid = match result {
                    Ok((agents, version)) => {
                        s.version = Some(version);
                        s.observation.commit(agents, now).is_ok()
                    }
                    Err(_) => false,
                };
                if valid {
                    s.errors = 0;
                    s.absent_since = None;
                    self.counters.snapshot_valid += 1;
                    self.last_useful = now;
                } else {
                    s.observation.fail();
                    s.version = None;
                    s.generation += 1;
                    s.errors += 1;
                    self.counters.snapshot_failed += 1;
                }
                if s.dirty {
                    s.dirty = false;
                    s.due = now;
                    s.trigger = "hook";
                } else {
                    s.due = now + if valid { 2000 } else { backoff(s.errors) };
                    s.trigger = "poll";
                }
            }
            ResultEvent::Native {
                generation,
                owned,
                state,
                failed,
                acquired,
                released,
                lost,
                issue,
            } => {
                self.native_busy = false;
                self.native.owned = owned;
                self.native.request_state = state;
                if acquired {
                    self.counters.native_acquires += 1;
                }
                if released {
                    self.counters.native_releases += 1;
                }
                if failed {
                    self.counters.native_failures += 1;
                }
                if lost {
                    self.counters.backend_losses += 1;
                }
                if !owned {
                    self.native_obsolete = false;
                }
                if generation != self.native_generation {
                    self.native_obsolete = owned;
                    self.counters.old_results_ignored += 1;
                    self.native_due = now;
                } else if failed {
                    self.native.last_error = issue.or_else(|| {
                        Some(Issue::new(
                            "backend_unavailable",
                            "Native operation failed; request is not confirmed",
                        ))
                    });
                    self.native_errors += 1;
                    self.native_due = now + backoff(self.native_errors);
                } else {
                    self.native.last_error = None;
                    self.native_errors = 0;
                    self.native_due = now + 1000;
                }
                if self.native.request_state == "suppressed" {
                    self.native.last_error = Some(Issue::new(
                        "desktop_suppressed",
                        "Desktop user suppression is respected; allow the existing request in desktop settings",
                    ));
                }
            }
        }
    }
    fn power_event(
        &mut self,
        event: backend::PowerEvent,
        now: u64,
        native: &mpsc::Sender<NativeCommand>,
    ) {
        self.suspended = matches!(event, backend::PowerEvent::Suspend);
        self.wake(now);
        self.evaluate(now, native);
    }
    fn observe_gap(&mut self, now: u64) {
        if now.saturating_sub(self.last_tick) > CHECK_FRESH_MS {
            self.suspended = false;
            self.wake(now);
            self.last_tick = now;
        }
    }
    fn wake(&mut self, now: u64) {
        self.revalidation_until = now + 5000;
        self.native_generation += 1;
        self.native_due = now;
        for job in std::mem::take(&mut self.snapshot_jobs).values() {
            job.abort();
        }
        for job in std::mem::take(&mut self.root_jobs).values() {
            job.abort();
        }
        self.snapshot_inflight = 0;
        self.root_inflight = 0;
        for r in self.roots.values_mut() {
            r.generation += 1;
            r.busy = false;
            r.healthy = false;
            r.enabled = None;
            r.due = now;
        }
        for s in self.servers.values_mut() {
            s.generation += 1;
            s.busy = false;
            s.version = None;
            s.observation.fail();
            s.due = now;
            s.trigger = "resume";
        }
        self.logger.transition("revalidate_after_execution_gap");
    }
    fn evaluate(&mut self, now: u64, native: &mpsc::Sender<NativeCommand>) {
        let selected = if self.conflict {
            Err(Issue::new(
                "desktop_context_conflict",
                "Conflicting desktop contexts",
            ))
        } else {
            backend::select(&self.config.config, &self.desktop)
        };
        if selected.as_ref().ok() != self.selected.as_ref() {
            self.native_generation += 1;
            self.selected = selected.as_ref().ok().cloned();
            self.native_due = now;
        }
        let observation = self.observation(now);
        let servers: Vec<_> = self
            .servers
            .values()
            .map(|s| s.observation.clone())
            .collect();
        let (fresh, retained) = self.eligibility(now);
        let decision = if self.suspended {
            Decision::off("shutdown")
        } else if !self.config.valid || !fresh && !(retained && self.native.owned) {
            Decision::off("ineligible")
        } else {
            self.policy.evaluate(
                &observation,
                &servers,
                now,
                self.config.config.paused,
                fresh,
                self.native.owned,
                self.config.config.release_delay_secs,
            )
        };
        if decision.desired != self.decision.desired {
            self.native_generation += 1;
            self.native_due = now;
            self.logger.transition(if decision.desired {
                "request_desired"
            } else {
                "request_withdrawn"
            });
        }
        self.decision = decision;
        self.seq += 1;
        self.evaluated_at = clock::unix_ms();
        if let Err(i) = selected {
            self.native.last_error = Some(i);
        }
        if self.native_busy {
            return;
        }
        let changing = self.native.backend.as_deref() != self.selected.as_ref().map(Kind::name);
        let operation = if self.native.owned
            && (!self.decision.desired
                || changing
                || self.native_obsolete
                || self.revalidation_until > now && !fresh)
        {
            self.native.request_state = "releasing".into();
            Some(NativeCommand::Release {
                generation: self.native_generation,
            })
        } else if self.decision.desired
            && !self.native.owned
            && now >= self.native_due
            && let Some(kind) = self.selected.clone()
        {
            self.native.backend = Some(kind.name().into());
            self.native.request_state = "pending".into();
            Some(NativeCommand::Acquire {
                generation: self.native_generation,
                kind,
            })
        } else if self.native.owned && now >= self.native_due {
            Some(NativeCommand::Check {
                generation: self.native_generation,
            })
        } else {
            None
        };
        if let Some(operation) = operation
            && native.try_send(operation).is_ok()
        {
            self.native_busy = true;
        }
    }
    fn retire_and_exit(&mut self, now: u64) -> bool {
        let complete = self.coverage(now);
        let relevant: BTreeSet<_> = self
            .roots
            .values()
            .flat_map(|r| r.endpoints.iter().cloned())
            .chain(self.config.config.additional_endpoints.iter().map(|e| {
                crate::herdr::discovery::normalize_endpoint(e).unwrap_or_else(|_| e.clone())
            }))
            .collect();
        if complete {
            self.servers.retain(|e, s| {
                if !relevant.contains(e) && s.transport_absent && !s.observation.current(now) {
                    let t = s.absent_since.get_or_insert(now);
                    now.saturating_sub(*t) < RETAIN_MS
                } else {
                    s.absent_since = None;
                    true
                }
            });
        }
        if now < self.revalidation_until {
            return false;
        }
        let (fresh, retained) = self.eligibility(now);
        if !self.roots.is_empty() && !fresh && !retained {
            return true;
        }
        if complete && self.servers.is_empty() {
            let t = self.gone_since.get_or_insert(now);
            if now.saturating_sub(*t) >= 30000 {
                return true;
            }
        } else {
            self.gone_since = None;
        }
        now.saturating_sub(self.last_useful) >= 120000
    }
}
fn backoff(errors: u32) -> u64 {
    match errors {
        0 | 1 => 1000,
        2 => 2000,
        _ => 5000,
    }
}
pub async fn serve(paths: Paths) -> anyhow::Result<()> {
    serve_with(paths, Box::new(NativeBackend)).await
}
/// Test injection preserves the real scheduler, parser, policy, and status transport.
pub async fn serve_with(paths: Paths, backend: Box<dyn PowerBackend>) -> anyhow::Result<()> {
    let Some(owner) = Owner::claim(paths.clone())? else {
        return Ok(());
    };
    let mut c = Controller::new(paths)?;
    let (incoming_tx, mut incoming_rx) = mpsc::channel(16);
    let (result_tx, mut results) = mpsc::channel(64);
    let mut power = backend::power_events()?;
    let native = native_worker(result_tx.clone(), backend);
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let stop = async {
        #[cfg(unix)]
        {
            tokio::select! {_=tokio::signal::ctrl_c()=>(),_=terminate.recv()=>()};
        }
        #[cfg(windows)]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    };
    tokio::pin!(stop);
    let listener = ipc::listen(&owner.listener, incoming_tx);
    tokio::pin!(listener);
    loop {
        tokio::select! {
            _=&mut stop=>break,
            _=&mut listener=>break,
            Some(message)=incoming_rx.recv()=>{c.control(message,clock::now_ms(),&native);},
            Some(result)=results.recv()=>{c.result(result,clock::now_ms());c.evaluate(clock::now_ms(),&native);},
            Some(event)=power.recv()=>{c.power_event(event,clock::now_ms(),&native);},
            _=tick.tick()=>{let now=clock::now_ms();c.observe_gap(now);c.last_tick=now;c.schedule(now,&result_tx);c.evaluate(now,&native);if c.retire_and_exit(now) {break;}},
        }
    }
    c.logger.transition("monitor_stopping");
    let (done, wait) = tokio::sync::oneshot::channel();
    let _ = native.send(NativeCommand::Stop { done }).await;
    let _ = wait.await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller(dir: &std::path::Path) -> Controller {
        let paths = Paths {
            config: dir.join("config.toml"),
            state: dir.join("logs"),
            runtime: dir.join("runtime"),
            endpoint: dir.join("control.sock").to_string_lossy().into_owned(),
        };
        let mut c = Controller::new(paths).unwrap();
        c.started = 0;
        c.last_tick = 0;
        c.last_useful = 0;
        c.revalidation_until = 0;
        c
    }
    fn registration(dir: &std::path::Path, n: &str) -> Registration {
        Registration {
            endpoint: dir.join(n).to_string_lossy().into_owned(),
            herdr_bin: std::env::current_exe().unwrap(),
            plugin_root: dir.into(),
            env: Default::default(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        }
    }
    #[tokio::test]
    async fn failed_config_operations_remain_in_shared_status_until_successful_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = controller(dir.path());
        let original = std::fs::read_to_string(&c.paths.config).unwrap();
        let (native, _) = mpsc::channel(8);
        std::fs::write(&c.paths.config, "invalid").unwrap();
        let (reply, rx) = tokio::sync::oneshot::channel();
        c.control(
            Incoming {
                request: ipc::Request {
                    protocol_version: 1,
                    request_id: "reload".into(),
                    operation: Operation::ReloadSettings,
                },
                reply,
            },
            1,
            &native,
        );
        assert!(rx.await.unwrap().error.is_some());
        for _ in 0..2 {
            let (reply, rx) = tokio::sync::oneshot::channel();
            c.control(
                Incoming {
                    request: ipc::Request {
                        protocol_version: 1,
                        request_id: "status".into(),
                        operation: Operation::GetStatus { details: false },
                    },
                    reply,
                },
                2,
                &native,
            );
            let status = Status::from_wire(rx.await.unwrap().status).unwrap();
            assert!(
                status
                    .diagnostics
                    .unwrap()
                    .issues
                    .iter()
                    .any(|i| i.code == "config_write_or_reload_failed")
            );
        }
        std::fs::write(&c.paths.config, format!("{original}\n# external edit\n")).unwrap();
        let (reply, rx) = tokio::sync::oneshot::channel();
        c.control(
            Incoming {
                request: ipc::Request {
                    protocol_version: 1,
                    request_id: "apply".into(),
                    operation: Operation::ApplySettings {
                        patch: ipc::SettingsPatch {
                            release_delay_secs: Some(6),
                            ..Default::default()
                        },
                    },
                },
                reply,
            },
            3,
            &native,
        );
        assert!(rx.await.unwrap().error.is_some());
        assert_eq!(c.config.config.release_delay_secs, 5);
        assert!(
            c.status(4)
                .diagnostics
                .unwrap()
                .issues
                .iter()
                .any(|i| i.code == "config_write_or_reload_failed")
        );
        c.config.reload().unwrap();
        assert!(
            !c.status(5)
                .diagnostics
                .unwrap()
                .issues
                .iter()
                .any(|i| i.code == "config_write_or_reload_failed")
        );
    }
    #[tokio::test]
    async fn short_suspend_resume_releases_and_requires_fresh_root_and_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = controller(dir.path());
        c.config.config.linux.backend = "hypridle".into();
        c.config.config.linux.hypridle_integration_confirmed = true;
        let registration = registration(dir.path(), "a.sock");
        let endpoint = registration.endpoint.clone();
        let key = registration.root_key();
        c.register(registration, 0).unwrap();
        c.result(
            ResultEvent::Root {
                key: key.clone(),
                generation: 0,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            0,
        );
        let snapshot = |generation| ResultEvent::Snapshot {
            endpoint: endpoint.clone(),
            generation,
            request: 0,
            result: Ok((
                vec![Agent {
                    terminal_id: "t".into(),
                    agent_status: "working".into(),
                }],
                "0.9.3".into(),
            )),
        };
        c.result(snapshot(0), 0);
        let (tx, mut commands) = mpsc::channel(8);
        c.evaluate(1, &tx);
        let NativeCommand::Acquire { generation, .. } = commands.recv().await.unwrap() else {
            panic!("initial acquire required");
        };
        let result = |generation, owned, acquired, released| ResultEvent::Native {
            generation,
            owned,
            state: if owned { "accepted" } else { "none" }.into(),
            failed: false,
            acquired,
            released,
            lost: false,
            issue: None,
        };
        c.result(result(generation, true, true, false), 2);
        // Milliseconds, not the 10-second fallback gap threshold.
        c.power_event(backend::PowerEvent::Suspend, 3, &tx);
        let NativeCommand::Release { generation } = commands.recv().await.unwrap() else {
            panic!("suspend must release");
        };
        c.result(result(generation, false, false, true), 4);
        c.power_event(backend::PowerEvent::Resume, 5, &tx);
        assert!(commands.try_recv().is_err());
        c.result(snapshot(0), 6);
        c.result(
            ResultEvent::Root {
                key: key.clone(),
                generation: 0,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            6,
        );
        c.evaluate(6, &tx);
        assert!(commands.try_recv().is_err());
        assert_eq!(c.counters.old_results_ignored, 2);
        let root_epoch = c.roots[&key].generation;
        let server_epoch = c.servers[&endpoint].generation;
        c.result(
            ResultEvent::Root {
                key,
                generation: root_epoch,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            7,
        );
        c.evaluate(7, &tx);
        assert!(commands.try_recv().is_err());
        c.result(snapshot(server_epoch), 8);
        c.evaluate(8, &tx);
        let NativeCommand::Acquire { generation, .. } = commands.recv().await.unwrap() else {
            panic!("fresh post-resume round must reacquire");
        };
        c.result(result(generation, true, true, false), 9);
        assert_eq!(c.counters.native_acquires, 2);
        assert_eq!(c.counters.native_releases, 1);
    }
    #[tokio::test]
    async fn registered_endpoints_survive_empty_discovery_and_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = controller(dir.path());
        let a = registration(dir.path(), "a.sock");
        let b = registration(dir.path(), "b.sock");
        c.register(a.clone(), 0).unwrap();
        c.register(b.clone(), 0).unwrap();
        let key = a.root_key();
        c.result(
            ResultEvent::Root {
                key: key.clone(),
                generation: 0,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            1,
        );
        assert_eq!(c.roots[&key].registered.len(), 2);
        assert!(c.roots[&key].registered.contains(&b.endpoint));
        assert!(c.roots[&key].endpoints.is_empty());
        c.servers.get_mut(&a.endpoint).unwrap().observation.fail();
        c.servers
            .get_mut(&b.endpoint)
            .unwrap()
            .observation
            .commit(
                vec![Agent {
                    terminal_id: "b".into(),
                    agent_status: "working".into(),
                }],
                2,
            )
            .unwrap();
        assert_eq!(c.eligibility(2), (true, true));
        assert_eq!(c.observation(2).work, Work::Working);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn configured_alias_and_canonical_bootstrap_have_one_identity() {
        let dir = tempfile::tempdir().unwrap();
        let actual = dir.path().join("actual");
        std::fs::create_dir(&actual).unwrap();
        std::os::unix::fs::symlink(&actual, dir.path().join("alias")).unwrap();
        let mut c = controller(dir.path());
        let r = registration(&actual, "a.sock");
        c.register(r, 0).unwrap();
        c.config.config.additional_endpoints = vec![
            dir.path()
                .join("alias/a.sock")
                .to_string_lossy()
                .into_owned(),
        ];
        let (tx, _) = mpsc::channel(32);
        c.schedule(0, &tx);
        assert_eq!(c.servers.len(), 1);
        assert_eq!(c.snapshot_inflight, 1);
        for h in c.snapshot_jobs.values().chain(c.root_jobs.values()) {
            h.abort();
        }
    }
    #[tokio::test]
    async fn queued_pre_gap_results_are_invalidated_before_tick() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = controller(dir.path());
        let r = registration(dir.path(), "a.sock");
        let e = r.endpoint.clone();
        let key = r.root_key();
        c.register(r, 0).unwrap();
        c.result(
            ResultEvent::Root {
                key: key.clone(),
                generation: 0,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            40000,
        );
        c.result(
            ResultEvent::Snapshot {
                endpoint: e.clone(),
                generation: 0,
                request: 0,
                result: Ok((
                    vec![Agent {
                        terminal_id: "t".into(),
                        agent_status: "working".into(),
                    }],
                    "0.9.3".into(),
                )),
            },
            40000,
        );
        let (tx, mut rx) = mpsc::channel(4);
        c.evaluate(40000, &tx);
        assert!(!c.decision.desired);
        assert!(rx.try_recv().is_err());
        assert_eq!(c.counters.old_results_ignored, 2);
        assert!(c.servers[&e].observation.agents.is_empty());
        assert_eq!(c.revalidation_until, 45000);
    }
    #[tokio::test]
    async fn pure_query_never_invalidates_generations_or_schedules_io() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = controller(dir.path());
        let (tx, mut native) = mpsc::channel(4);
        let (reply, rx) = tokio::sync::oneshot::channel();
        c.control(
            Incoming {
                request: ipc::Request {
                    protocol_version: 1,
                    request_id: "q".into(),
                    operation: Operation::GetStatus { details: false },
                },
                reply,
            },
            50000,
            &tx,
        );
        assert!(rx.await.unwrap().status["available"].as_bool().unwrap());
        assert_eq!(c.last_tick, 0);
        assert_eq!(c.native_generation, 0);
        assert!(native.try_recv().is_err());
        assert_eq!(c.counters.status_reads, 1);
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct SlowFactory {
        started: Arc<tokio::sync::Notify>,
        gate: Arc<tokio::sync::Notify>,
        live: Arc<AtomicUsize>,
        fails: Arc<AtomicUsize>,
    }
    struct FakeRequest {
        live: Arc<AtomicUsize>,
        fails: Arc<AtomicUsize>,
        owned: bool,
    }
    #[async_trait::async_trait]
    impl PowerBackend for SlowFactory {
        async fn acquire(&self, _: &Kind) -> anyhow::Result<Box<dyn OwnedRequest>> {
            self.started.notify_one();
            self.gate.notified().await;
            self.live.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeRequest {
                live: self.live.clone(),
                fails: self.fails.clone(),
                owned: true,
            }))
        }
    }
    #[async_trait::async_trait]
    impl OwnedRequest for FakeRequest {
        async fn state(&mut self) -> anyhow::Result<String> {
            Ok(if self.owned { "accepted" } else { "none" }.into())
        }
        async fn release(&mut self) -> anyhow::Result<()> {
            if self
                .fails
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                anyhow::bail!("injected release failure");
            }
            if self.owned {
                self.owned = false;
                self.live.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }
    impl Drop for FakeRequest {
        fn drop(&mut self) {
            if self.owned {
                self.live.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }
    #[tokio::test]
    async fn delayed_acquire_is_released_after_pause_invalidates_generation() {
        let dir = tempfile::tempdir().unwrap();
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
        let mut c = Controller::new(paths).unwrap();
        c.started = 0;
        c.last_tick = 0;
        c.desktop = "Hyprland".into();
        c.config.config.linux.backend = "hypridle".into();
        c.config.config.linux.hypridle_integration_confirmed = true;
        let r = Registration {
            endpoint: dir.path().join("a.sock").to_string_lossy().into_owned(),
            herdr_bin: std::env::current_exe().unwrap(),
            plugin_root: dir.path().into(),
            env: Default::default(),
            desktop: "Hyprland".into(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        };
        let key = r.root_key();
        let endpoint = r.endpoint.clone();
        c.register(r, 0).unwrap();
        c.result(
            ResultEvent::Root {
                key,
                generation: 0,
                discovery: Ok(vec![]),
                eligibility: Ok(true),
            },
            0,
        );
        c.servers
            .get_mut(&endpoint)
            .unwrap()
            .observation
            .commit(
                vec![Agent {
                    terminal_id: "t".into(),
                    agent_status: "working".into(),
                }],
                0,
            )
            .unwrap();
        let started = Arc::new(tokio::sync::Notify::new());
        let gate = Arc::new(tokio::sync::Notify::new());
        let live = Arc::new(AtomicUsize::new(0));
        let (tx, mut results) = mpsc::channel(8);
        let commands = native_worker(
            tx,
            Box::new(SlowFactory {
                started: started.clone(),
                gate: gate.clone(),
                live: live.clone(),
                fails: Default::default(),
            }),
        );
        c.evaluate(1, &commands);
        started.notified().await;
        let (reply, rx) = tokio::sync::oneshot::channel();
        c.control(
            Incoming {
                request: ipc::Request {
                    protocol_version: 1,
                    request_id: "pause".into(),
                    operation: Operation::SetPaused { paused: true },
                },
                reply,
            },
            2,
            &commands,
        );
        assert!(rx.await.unwrap().error.is_none());
        assert!(!c.decision.desired);
        assert_eq!(c.status(2).inhibition.resource_owned, None);
        gate.notify_one();
        let event = results.recv().await.unwrap();
        c.result(event, 3);
        c.evaluate(3, &commands);
        let event = results.recv().await.unwrap();
        c.result(event, 4);
        assert_eq!(live.load(Ordering::SeqCst), 0);
        assert!(!c.native.owned);
        assert_eq!(c.counters.native_acquires, 1);
        assert_eq!(c.counters.native_releases, 1);
        assert!(c.counters.old_results_ignored > 0);
        let (done, rx) = tokio::sync::oneshot::channel();
        commands.send(NativeCommand::Stop { done }).await.unwrap();
        rx.await.unwrap();
    }
    #[tokio::test]
    async fn failed_release_retains_ownership_until_confirmed_cleanup() {
        let (tx, mut results) = mpsc::channel(8);
        let gate = Arc::new(tokio::sync::Notify::new());
        gate.notify_one();
        let live = Arc::new(AtomicUsize::new(0));
        let commands = native_worker(
            tx,
            Box::new(SlowFactory {
                started: Default::default(),
                gate,
                live: live.clone(),
                fails: Arc::new(AtomicUsize::new(1)),
            }),
        );
        commands
            .send(NativeCommand::Acquire {
                generation: 0,
                kind: Kind::Hypridle,
            })
            .await
            .unwrap();
        results.recv().await.unwrap();
        commands
            .send(NativeCommand::Release { generation: 1 })
            .await
            .unwrap();
        assert!(matches!(
            results.recv().await.unwrap(),
            ResultEvent::Native {
                owned: true,
                failed: true,
                released: false,
                ..
            }
        ));
        assert_eq!(live.load(Ordering::SeqCst), 1);
        commands
            .send(NativeCommand::Release { generation: 2 })
            .await
            .unwrap();
        assert!(matches!(
            results.recv().await.unwrap(),
            ResultEvent::Native {
                owned: false,
                failed: false,
                released: true,
                ..
            }
        ));
        assert_eq!(live.load(Ordering::SeqCst), 0);
        let (done, rx) = tokio::sync::oneshot::channel();
        commands.send(NativeCommand::Stop { done }).await.unwrap();
        rx.await.unwrap();
    }
}
