//! Update publication is distinct from monitor activation: no saved Pause changes.
use super::cache::{Binary, Stamp, fingerprint};
use crate::herdr::protocol::Plugin;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Publication {
    pub executable: PathBuf,
    pub digest: String,
    pub plugin_root: PathBuf,
    pub expected_commit: Option<String>,
}
impl Publication {
    pub fn ready(&self, plugin: &Plugin) -> anyhow::Result<bool> {
        if plugin.plugin_id != crate::herdr::discovery::PLUGIN_ID
            || !plugin.enabled
            || Path::new(&plugin.plugin_root) != self.plugin_root
            || self.expected_commit.as_ref().is_some_and(|commit| {
                plugin
                    .source
                    .as_ref()
                    .and_then(|s| s.resolved_commit.as_ref())
                    != Some(commit)
            })
        {
            return Ok(false);
        }
        match fingerprint(&self.executable) {
            Ok(digest) => Ok(digest == self.digest),
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug)]
struct Observed {
    path: PathBuf,
    stamp: Stamp,
    digest: String,
}
#[derive(Debug)]
pub struct SourceTracker {
    loaded: String,
    source: PathBuf,
    roots: BTreeMap<String, Observed>,
    retry_paths: std::collections::BTreeSet<PathBuf>,
    fingerprints_read: u64,
    rejected: BTreeSet<String>,
}
impl SourceTracker {
    pub fn new(loaded: String, source: PathBuf) -> Self {
        Self {
            loaded,
            source,
            roots: BTreeMap::new(),
            retry_paths: Default::default(),
            fingerprints_read: 0,
            rejected: Default::default(),
        }
    }
    pub fn reject(&mut self, digest: String) {
        self.rejected.insert(digest);
    }
    pub fn fingerprints_read(&self) -> u64 {
        self.fingerprints_read
    }
    pub fn retry(&mut self, path: &Path) {
        self.roots.retain(|_, root| root.path != path);
        self.retry_paths.insert(path.into());
    }
    pub fn inspect(&mut self, key: &str, path: &Path) -> anyhow::Result<Option<Binary>> {
        let stamp = match Stamp::read(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let old = self.roots.get(key);
        if old.is_some_and(|o| o.path == path && o.stamp == stamp) {
            return Ok(None);
        }
        let digest = fingerprint(path)?;
        self.fingerprints_read += 1;
        let changed = old.map_or(
            (path == self.source || self.retry_paths.contains(path)) && digest != self.loaded,
            |o| o.digest != digest,
        );
        self.roots.insert(
            key.into(),
            Observed {
                path: path.into(),
                stamp,
                digest: digest.clone(),
            },
        );
        Ok(
            (changed && digest != self.loaded && !self.rejected.contains(&digest)).then(|| {
                Binary {
                    path: path.into(),
                    digest,
                }
            }),
        )
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeInfo {
    pub paths: super::paths::Paths,
    pub executable: PathBuf,
    pub source: PathBuf,
    pub digest: String,
    pub registrations: Vec<crate::herdr::discovery::Registration>,
    pub legacy_endpoints: Vec<String>,
    pub effective_pause: Option<bool>,
    #[serde(default)]
    pub rejected_updates: BTreeSet<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub paths: super::paths::Paths,
    pub registrations: Vec<crate::herdr::discovery::Registration>,
    pub legacy_endpoints: Vec<String>,
    pub effective_pause: Option<bool>,
    pub source: Option<PathBuf>,
    #[serde(default)]
    pub rejected_updates: BTreeSet<String>,
}
impl Handoff {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.paths.config.is_absolute()
                && self.paths.state.is_absolute()
                && self.paths.runtime.is_absolute(),
            "handoff paths must be absolute"
        );
        anyhow::ensure!(
            self.registrations.len() <= 1024 && self.legacy_endpoints.len() <= 1024,
            "handoff exceeds registration limit"
        );
        anyhow::ensure!(
            self.rejected_updates.len() <= 1024
                && self
                    .rejected_updates
                    .iter()
                    .all(|digest| digest.len() == 64
                        && digest.bytes().all(|b| b.is_ascii_hexdigit())),
            "invalid rejected update identities"
        );
        for r in &self.registrations {
            r.validate()?;
        }
        for e in &self.legacy_endpoints {
            crate::runtime::config::validate_endpoint(e)?;
        }
        Ok(())
    }
}

const PREPARE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
const PUBLICATION_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub paths: super::paths::Paths,
    pub candidate: Binary,
    pub publication: Publication,
    pub base: crate::herdr::discovery::Registration,
    pub handoff: Option<Handoff>,
    pub prepare: bool,
    pub direct: bool,
    pub discover_root: bool,
    pub source_root: Option<PathBuf>,
    pub timeout_ms: u64,
}

pub struct Ticket {
    child: std::process::Child,
    pub directory: PathBuf,
}
impl Ticket {
    pub fn exited(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }
}

pub fn pending(paths: &super::paths::Paths) -> anyhow::Result<bool> {
    let file = match super::singleton::coordination_file(&paths.runtime, "update.lock", false) {
        Ok(f) => f,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(false);
        }
        Err(e) => return Err(e),
    };
    match file.try_lock_shared() {
        Ok(()) => Ok(false),
        Err(std::fs::TryLockError::WouldBlock) => Ok(true),
        Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
    }
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("private file parent missing"))?;
    super::singleton::private_dir(parent)?;
    let bytes = serde_json::to_vec(value)?;
    anyhow::ensure!(bytes.len() <= 1024 * 1024, "handoff is too large");
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&bytes)?;
    #[cfg(windows)]
    super::windows_security::protect_path(file.path())?;
    file.persist(path)?;
    Ok(())
}
pub fn read_handoff(path: &Path) -> anyhow::Result<Handoff> {
    let boot: Handoff = read_json(path)?;
    boot.validate()?;
    Ok(boot)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let m = file.metadata()?;
    anyhow::ensure!(m.is_file() && m.len() <= 1024 * 1024, "unsafe handoff file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        anyhow::ensure!(
            m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
            "unsafe handoff permissions"
        );
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 1024 * 1024, "handoff is too large");
    Ok(serde_json::from_slice(&bytes)?)
}

async fn queue_lock(paths: &super::paths::Paths) -> anyhow::Result<std::fs::File> {
    let file = super::singleton::coordination_file(&paths.runtime, "queue.lock", true)?;
    let deadline = tokio::time::Instant::now() + PREPARE_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "activation queue deadline expired"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
/// A shared gate spans the whole bootstrap, closing the check/connect race.
/// An exclusive updater instead receives the activation in its persistent queue.
pub async fn activation_guard(
    paths: &super::paths::Paths,
    registration: &crate::herdr::discovery::Registration,
) -> anyhow::Result<Option<std::fs::File>> {
    registration.validate()?;
    let gate = super::singleton::coordination_file(&paths.runtime, "update.lock", true)?;
    loop {
        match gate.try_lock_shared() {
            Ok(()) => return Ok(Some(gate)),
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        let _queue = queue_lock(paths).await?;
        if !pending(paths)? {
            continue;
        }
        write_json(
            &paths
                .runtime
                .join("pending")
                .join(format!("{}.json", uuid::Uuid::new_v4())),
            registration,
        )?;
        return Ok(None);
    }
}
pub async fn queue_if_pending(
    paths: &super::paths::Paths,
    registration: &crate::herdr::discovery::Registration,
) -> anyhow::Result<bool> {
    Ok(activation_guard(paths, registration).await?.is_none())
}
fn queued_registrations(
    paths: &super::paths::Paths,
) -> anyhow::Result<Vec<(PathBuf, crate::herdr::discovery::Registration)>> {
    let entries = match std::fs::read_dir(paths.runtime.join("pending")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    entries
        .take(1024)
        .map(|entry| {
            let path = entry?.path();
            let registration: crate::herdr::discovery::Registration = read_json(&path)?;
            registration.validate()?;
            Ok((path, registration))
        })
        .collect()
}
// Remove a queued record only after its owner acknowledges the registration.
async fn replay_queued(paths: &super::paths::Paths) -> anyhow::Result<()> {
    for (path, registration) in queued_registrations(paths)? {
        let reply = super::ipc::query(
            &paths.endpoint,
            super::ipc::Operation::RegisterAndRefresh { registration },
        )
        .await
        .map_err(|e| anyhow::anyhow!("queued activation failed: {}", e.code))?;
        anyhow::ensure!(reply.error.is_none(), "queued activation was rejected");
        std::fs::remove_file(path)?;
    }
    Ok(())
}
pub async fn replay_pending(paths: &super::paths::Paths) -> anyhow::Result<()> {
    let _queue = queue_lock(paths).await?;
    replay_queued(paths).await
}
fn collect_queued(
    paths: &super::paths::Paths,
    registrations: &mut Vec<crate::herdr::discovery::Registration>,
) -> anyhow::Result<()> {
    for (_, registration) in queued_registrations(paths)? {
        if !registrations.contains(&registration) {
            registrations.push(registration);
        }
    }
    Ok(())
}

fn detached(
    binary: &Path,
    args: &[&std::ffi::OsStr],
    paths: &super::paths::Paths,
    context: Option<&crate::herdr::discovery::Registration>,
) -> anyhow::Result<std::process::Child> {
    use std::process::{Command, Stdio};
    super::singleton::private_dir(&paths.state)?;
    let log = crate::diagnostics::open_log(&paths.state)?;
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(&paths.state)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("HERDR_") {
            command.env_remove(key);
        }
    }
    command
        .env("HERDR_IDLE_INHIBITOR_CONFIG", &paths.config)
        .env("HERDR_IDLE_INHIBITOR_STATE", &paths.state);
    if let Some(context) = context {
        command.env("XDG_CURRENT_DESKTOP", &context.desktop);
        if let Some(bus) = &context.bus {
            command.env("DBUS_SESSION_BUS_ADDRESS", bus);
        } else {
            command.env_remove("DBUS_SESSION_BUS_ADDRESS");
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and this hook does not allocate.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000008 | 0x08000000);
    }
    Ok(command.spawn()?)
}

pub fn launch(plan: &Plan, helper: &Path) -> anyhow::Result<Ticket> {
    plan.base.validate()?;
    let directory = plan
        .paths
        .state
        .join("updates")
        .join(uuid::Uuid::new_v4().to_string());
    let path = directory.join("plan.json");
    write_json(&path, plan)?;
    let child = detached(
        helper,
        &[std::ffi::OsStr::new("_upgrade_worker"), path.as_os_str()],
        &plan.paths,
        plan.handoff.as_ref().and_then(|h| h.registrations.first()),
    )?;
    Ok(Ticket { child, directory })
}

async fn installed_plugin(
    base: &crate::herdr::discovery::Registration,
) -> anyhow::Result<Option<Plugin>> {
    let bytes = crate::herdr::discovery::bounded_command(
        &base.herdr_bin,
        &[
            "plugin",
            "list",
            "--plugin",
            crate::herdr::discovery::PLUGIN_ID,
            "--json",
        ],
        &base.env,
    )
    .await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let plugins: Vec<Plugin> = serde_json::from_value(value["result"]["plugins"].clone())?;
    Ok(plugins
        .into_iter()
        .find(|p| p.plugin_id == crate::herdr::discovery::PLUGIN_ID))
}

async fn probe(binary: &Path) -> anyhow::Result<bool> {
    use tokio::io::AsyncReadExt;
    async fn output(binary: &Path, argument: &str) -> anyhow::Result<(bool, Vec<u8>)> {
        let mut child = tokio::process::Command::new(binary)
            .arg(argument)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("probe stdout missing"))?;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut bytes = Vec::new();
            stdout.take(4097).read_to_end(&mut bytes).await?;
            anyhow::ensure!(bytes.len() <= 4096, "oversized executable probe");
            Ok((child.wait().await?.success(), bytes))
        })
        .await?
    }
    let (success, version) = output(binary, "--version").await?;
    anyhow::ensure!(
        success
            && std::str::from_utf8(&version)?
                .trim()
                .starts_with("herdr-idle-inhibitor "),
        "replacement is not a runnable Idle Inhibitor executable"
    );
    let (success, capabilities) = output(binary, "_capabilities").await?;
    if !success {
        return Ok(false);
    } // Older executable: existing _serve/RegisterAndRefresh protocol.
    let capabilities: serde_json::Value = serde_json::from_slice(&capabilities)?;
    anyhow::ensure!(
        capabilities["update_protocol"] == 1,
        "unsupported update protocol"
    );
    Ok(true)
}

fn find_herdr() -> anyhow::Result<PathBuf> {
    if let Some(bin) = std::env::var_os("HERDR_BIN_PATH") {
        return Ok(std::fs::canonicalize(bin)?);
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for name in ["herdr", "herdr.exe"] {
            let path = directory.join(name);
            if path.is_file() {
                return Ok(std::fs::canonicalize(path)?);
            }
        }
    }
    anyhow::bail!("Herdr must be available on PATH during source installation")
}
pub async fn prepare() -> anyhow::Result<()> {
    let source_root = std::env::current_dir()?;
    let base = crate::herdr::discovery::Registration::for_root(
        find_herdr()?,
        source_root.clone(),
        source_root
            .join("unused.sock")
            .to_string_lossy()
            .into_owned(),
    )?;
    // Linked source archives need no Git metadata. GitHub checkouts use their
    // resolved revision as an additional publication marker, even at one version.
    let mut git = tokio::process::Command::new("git");
    git.args(["rev-parse", "HEAD"]).kill_on_drop(true);
    let commit = tokio::time::timeout(std::time::Duration::from_secs(2), git.output())
        .await
        .ok()
        .and_then(Result::ok)
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    prepare_from(
        super::paths::Paths::get()?,
        std::env::current_exe()?,
        base,
        source_root,
        commit,
    )
    .await
}

async fn api(endpoint: &str, method: &str) -> anyhow::Result<serde_json::Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let bytes = crate::herdr::transport::call(
        endpoint,
        &id,
        method,
        serde_json::json!({"plugin_id":crate::herdr::discovery::PLUGIN_ID}),
    )
    .await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(
        value["id"] == id && value.get("error").is_none(),
        "Herdr rejected update coordination"
    );
    Ok(value)
}
async fn endpoint_plugin(endpoint: &str) -> anyhow::Result<Option<Plugin>> {
    let value = api(endpoint, "plugin.list").await?;
    let plugins: Vec<Plugin> = serde_json::from_value(value["result"]["plugins"].clone())?;
    Ok(plugins
        .into_iter()
        .find(|p| p.plugin_id == crate::herdr::discovery::PLUGIN_ID))
}
async fn root_registrations(
    base: &crate::herdr::discovery::Registration,
) -> anyhow::Result<Vec<crate::herdr::discovery::Registration>> {
    Ok(crate::herdr::discovery::discover(base)
        .await?
        .into_iter()
        .filter(|s| s.running)
        .map(|s| {
            let mut registration = base.clone();
            registration.endpoint = s.socket_path;
            registration
        })
        .collect())
}

async fn spawn_monitor(image: &Binary, boot: &Handoff, directory: &Path) -> anyhow::Result<()> {
    use super::ipc::{self, Operation};
    if boot.registrations.is_empty() && boot.legacy_endpoints.is_empty() {
        return Ok(());
    }
    boot.validate()?;
    let path = directory.join("resume.json");
    write_json(&path, boot)?;
    let mut child = detached(
        &image.path,
        &[std::ffi::OsStr::new("_resume"), path.as_os_str()],
        &boot.paths,
        boot.registrations.first(),
    )?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Ok(reply) =
            ipc::query(&boot.paths.endpoint, Operation::GetStatus { details: true }).await
        {
            anyhow::ensure!(
                reply
                    .details
                    .and_then(|d| d.runtime)
                    .is_some_and(|i| i.digest == image.digest),
                "a different monitor won update activation"
            );
            return Ok(());
        }
        anyhow::ensure!(
            child.try_wait()?.is_none(),
            "replacement monitor exited during startup"
        );
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "replacement startup deadline expired"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

/// The install caller only waits for preparation. This cache-resident process
/// holds the stable singleton during publication, so no old hook can become owner.
pub async fn worker(path: &Path) -> anyhow::Result<()> {
    use super::{
        ipc::{self, Operation},
        singleton::Owner,
    };
    let mut plan: Plan = read_json(path)?;
    plan.base.validate()?;
    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("plan parent missing"))?;
    let lock = super::singleton::coordination_file(&plan.paths.runtime, "update.lock", true)?;
    let lock_deadline = tokio::time::Instant::now() + PREPARE_TIMEOUT;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < lock_deadline,
            "another update is still in progress"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    eprintln!("update: preparation lock acquired");
    let mut previous: Option<(Binary, Handoff)> = None;
    let mut stopping = false;
    let mut cached_monitor = false;
    let mut same_code = false;
    let result: anyhow::Result<()> = async {
        // Probe the source, not just the cache copy: respect source execution policy.
        let modern = probe(&plan.publication.executable).await;
        let candidate_modern = if plan.prepare {
            probe(&plan.candidate.path).await?
        } else { modern? };
        anyhow::ensure!(candidate_modern, "Replacement does not support the automatic-update handoff protocol");
        anyhow::ensure!(fingerprint(&plan.candidate.path)? == plan.candidate.digest, "replacement cache identity changed");
        let mut boot = plan.handoff.clone().unwrap_or(Handoff { paths: plan.paths.clone(), registrations: vec![], legacy_endpoints: vec![], effective_pause: None, source: Some(plan.publication.executable.clone()), rejected_updates: Default::default() });
        eprintln!("update: candidate verified");
        let mut client_images = if plan.discover_root { vec![] } else { vec![plan.publication.executable.clone()] };
        match ipc::query(&plan.paths.endpoint, Operation::GetStatus { details: true }).await {
            Ok(reply) => {
                let info = reply.details.and_then(|details| details.runtime)
                    .ok_or_else(|| anyhow::anyhow!("monitor does not support automatic-update handover"))?;
                anyhow::ensure!(info.paths.endpoint == plan.paths.endpoint && info.paths.runtime == plan.paths.runtime, "monitor ownership namespace changed");
                cached_monitor = info.executable.starts_with(info.paths.state.join("bin"));
                same_code = cached_monitor && info.digest == plan.candidate.digest;
                plan.candidate = super::cache::stage(&plan.candidate.path, &info.paths.state)?;
                plan.paths = info.paths.clone();
                boot.paths = info.paths.clone();
                boot.source = Some(info.source.clone());
                boot.registrations = info.registrations;
                boot.legacy_endpoints = info.legacy_endpoints;
                boot.effective_pause = info.effective_pause;
                boot.rejected_updates = info.rejected_updates;
                client_images.extend(boot.registrations.iter().map(|r| super::cache::installed(&r.plugin_root)));
                previous = Some((Binary { path: info.executable, digest: info.digest }, boot.clone()));
                stopping = !cached_monitor;
                if stopping {
                    let reply = ipc::query(&plan.paths.endpoint, Operation::PrepareUpgrade).await.map_err(|e| anyhow::anyhow!("update control query failed: {}", e.code))?;
                    anyhow::ensure!(reply.error.is_none(), "monitor rejected update preparation");
                    // Capture registrations accepted since the initial status read.
                    if let Some(info) = reply.details.and_then(|d| d.runtime) {
                        boot.registrations = info.registrations;
                        boot.legacy_endpoints = info.legacy_endpoints;
                        boot.effective_pause = info.effective_pause;
                         boot.rejected_updates = info.rejected_updates;
                        if let Some(previous) = &mut previous { previous.1 = boot.clone(); }
                    }
                }
            }
            Err(e) if e.exit == 3 => (),
            Err(e) => anyhow::bail!("cannot coordinate monitor update: {}", e.code),
        }
        plan.paths = boot.paths.clone();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(40);
        eprintln!("update: previous monitor described; cached={cached_monitor}");
        let owner = if cached_monitor { None } else { Some(loop {
            if let Some(owner) = Owner::claim(plan.paths.clone())? { break owner; }
            anyhow::ensure!(tokio::time::Instant::now() < deadline, "previous monitor did not release ownership");
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }) };
        #[cfg(windows)]
        super::update_windows::close_installed_clients(&client_images).await?;
        #[cfg(not(windows))]
        let _ = client_images;
        write_json(&directory.join("ready.json"), &true)?;
        eprintln!("update: ready for installation publication");
        {
        let (tx, mut incoming) = tokio::sync::mpsc::channel(16);
        let listener = async {
            if let Some(owner) = &owner { let _ = ipc::listen(&owner.listener, tx).await; }
            else { std::future::pending::<()>().await; }
        };
        tokio::pin!(listener);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(plan.timeout_ms);
        let mut cached_stamp = None;
        let mut cached_digest = None;
        let mut poll = tokio::time::interval(std::time::Duration::from_millis(100));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = poll.tick() => {
                    let plugin = if plan.direct { endpoint_plugin(&plan.base.endpoint).await? } else { installed_plugin(&plan.base).await? };
                    if let Some(plugin) = plugin {
                        let linked_source = plugin.source.as_ref().and_then(|s| s.resolved_commit.as_ref()).is_none() && plan.source_root.as_ref().is_some_and(|root| root == Path::new(&plugin.plugin_root));
                        if (plan.discover_root || linked_source) && Path::new(&plugin.plugin_root).is_absolute() {
                            plan.publication.plugin_root = plugin.plugin_root.clone().into();
                            plan.publication.executable = super::cache::installed(&plan.publication.plugin_root);
                            plan.base.plugin_root = plan.publication.plugin_root.clone();
                        }
                        let registry_matches = Path::new(&plugin.plugin_root) == plan.publication.plugin_root && (linked_source || plan.publication.expected_commit.as_ref().is_none_or(|commit| plugin.source.as_ref().and_then(|s| s.resolved_commit.as_ref()) == Some(commit)));
                        if registry_matches && let Ok(stamp) = Stamp::read(&plan.publication.executable) {
                            if cached_stamp.as_ref() != Some(&stamp) {
                                cached_digest = Some(fingerprint(&plan.publication.executable)?);
                                cached_stamp = Some(stamp);
                            }
                            if cached_digest.as_ref() == Some(&plan.publication.digest) { break; }
                        }
                    }
                    anyhow::ensure!(tokio::time::Instant::now() < deadline, "installation did not publish the prepared executable");
                }
                _ = &mut listener => anyhow::bail!("update listener stopped"),
                Some(message) = incoming.recv() => {
                    if let Operation::RegisterAndRefresh { registration } = message.request.operation
                        && registration.validate().is_ok() && !boot.registrations.contains(&registration) { boot.registrations.push(registration); }
                    // Dropping the reply closes the stream: status means unavailable,
                    // never an invented available snapshot or a zero-agent observation.
                },
            }
        }
        }
        eprintln!("update: installation publication verified at {}", plan.publication.plugin_root.display());
        if cached_monitor && !same_code {
            let reply = ipc::query(&plan.paths.endpoint, Operation::PrepareUpgrade).await.map_err(|e| anyhow::anyhow!("update control query failed: {}", e.code))?;
            anyhow::ensure!(reply.error.is_none(), "monitor rejected update preparation");
            stopping = true;
            if let Some(info) = reply.details.and_then(|d| d.runtime) {
                boot.registrations = info.registrations;
                boot.legacy_endpoints = info.legacy_endpoints;
                boot.effective_pause = info.effective_pause;
                         boot.rejected_updates = info.rejected_updates;
                if let Some(previous) = &mut previous { previous.1 = boot.clone(); }
            }
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(40);
            loop {
                if let Some(owner) = Owner::claim(plan.paths.clone())? { drop(owner); break; }
                anyhow::ensure!(tokio::time::Instant::now() < deadline, "previous monitor did not release ownership");
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        }
        drop(owner);
        // Add the install caller's root (including first installation) without
        // dropping registrations belonging to other configuration directories.
        for registration in if plan.direct { vec![] } else { root_registrations(&plan.base).await? } {
            if !boot.registrations.contains(&registration) { boot.registrations.push(registration); }
        }
        collect_queued(&plan.paths, &mut boot.registrations)?;
        boot.source = Some(plan.publication.executable.clone());
        eprintln!("update: starting monitor with {} known roots", boot.registrations.len());
        if !same_code { spawn_monitor(&plan.candidate, &boot, directory).await?; }
        // Serialize the last drain with enqueue, then release update.lock while
        // holding queue.lock. A startup/event can never fall between drain and unlock.
        let _queue = queue_lock(&plan.paths).await?;
        collect_queued(&plan.paths, &mut boot.registrations)?;
        if !boot.registrations.is_empty() || !boot.legacy_endpoints.is_empty() {
            if ipc::query(&plan.paths.endpoint, Operation::GetStatus { details: false }).await.is_err() {
                spawn_monitor(&plan.candidate, &boot, directory).await?;
            }
            for registration in &boot.registrations {
                let reply = ipc::query(&plan.paths.endpoint, Operation::RegisterAndRefresh { registration: registration.clone() }).await.map_err(|e| anyhow::anyhow!("queued activation failed: {}", e.code))?;
                anyhow::ensure!(reply.error.is_none(), "queued activation was rejected");
            }
        }
        replay_queued(&plan.paths).await?;
        std::fs::File::unlock(&lock)?;
        write_json(&directory.join("complete.json"), &true)?;
        let _ = std::fs::remove_file(plan.paths.runtime.join("update-error.json"));
        Ok(())
    }.await;
    if let Err(error) = result {
        let message = crate::diagnostics::sanitize(&error.to_string());
        write_json(&directory.join("error.json"), &message)?;
        write_json(&plan.paths.runtime.join("update-error.json"), &message)?;
        // All failures, including probes before owner inspection, finalize the
        // activation queue. Keep unacknowledged records for the next bootstrap.
        let _queue = queue_lock(&plan.paths).await?;
        let recovery: anyhow::Result<()> = async {
            if stopping && let Some((image, mut boot)) = previous {
                collect_queued(&boot.paths, &mut boot.registrations)?;
                boot.rejected_updates.insert(plan.candidate.digest.clone());
                // Never remove a live lock or start a second native owner.
                if ipc::query(
                    &boot.paths.endpoint,
                    Operation::GetStatus { details: false },
                )
                .await
                .is_err()
                {
                    spawn_monitor(&image, &boot, directory).await?;
                }
            }
            replay_queued(&plan.paths).await
        }
        .await;
        std::fs::File::unlock(&lock)?;
        if let Err(recovery) = recovery {
            eprintln!("update: recovery incomplete; queued activations retained: {recovery}");
        }
        return Err(error);
    }
    Ok(())
}

/// Produce an update from explicit runtime inputs; the CLI only captures these.
pub async fn prepare_from(
    paths: super::paths::Paths,
    source: PathBuf,
    mut base: crate::herdr::discovery::Registration,
    source_root: PathBuf,
    expected_commit: Option<String>,
) -> anyhow::Result<()> {
    base.validate()?;
    let installed = installed_plugin(&base).await?;
    let discover_root = installed.is_none();
    if let Some(plugin) = installed {
        base.plugin_root = plugin.plugin_root.into();
    }
    let state = paths.state.clone();
    let image = tokio::task::spawn_blocking(move || super::cache::stage(&source, &state)).await??;
    let plan = Plan {
        publication: Publication {
            executable: super::cache::installed(&base.plugin_root),
            digest: image.digest.clone(),
            plugin_root: base.plugin_root.clone(),
            expected_commit,
        },
        paths,
        candidate: image.clone(),
        base,
        handoff: None,
        prepare: true,
        direct: false,
        discover_root,
        source_root: Some(std::fs::canonicalize(source_root)?),
        timeout_ms: PUBLICATION_TIMEOUT_MS,
    };
    let mut ticket = launch(&plan, &image.path)?;
    let deadline = tokio::time::Instant::now() + PREPARE_TIMEOUT;
    loop {
        if ticket.directory.join("ready.json").is_file() {
            return Ok(());
        }
        if let Ok(error) = read_json::<String>(&ticket.directory.join("error.json")) {
            anyhow::bail!("update preparation failed: {error}");
        }
        if ticket.exited()?.is_some() {
            anyhow::bail!("update preparation process exited; inspect application logs");
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "update preparation deadline expired"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}
