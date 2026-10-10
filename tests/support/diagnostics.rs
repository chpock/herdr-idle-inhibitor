//! Test-only failure evidence. Healthy runs do no hashing/proc inspection or artifact copying.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Instant, SystemTime},
};

struct Process {
    label: String,
    command: Vec<String>,
    binary: PathBuf,
    child: Option<usize>,
    spawn_error: Option<String>,
    started: Instant,
}
#[derive(Default)]
struct Context {
    values: BTreeMap<String, Value>,
    order: BTreeMap<String, u64>,
    sequence: u64,
}
pub struct Diagnostics {
    directory: tempfile::TempDir,
    artifacts: PathBuf,
    children: Vec<Child>,
    processes: Vec<Process>,
    context: RefCell<Context>,
    captured: Option<PathBuf>,
    capture_error: Option<String>,
}
impl Diagnostics {
    pub fn new(directory: tempfile::TempDir, artifacts: Option<PathBuf>) -> Self {
        Self {
            directory,
            artifacts: artifacts
                .or_else(|| std::env::var_os("HERDR_TEST_ARTIFACTS").map(PathBuf::from))
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist/test-failures")
                }),
            children: vec![],
            processes: vec![],
            context: RefCell::new(Context::default()),
            captured: None,
            capture_error: None,
        }
    }
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
    pub fn spawn(&mut self, label: &str, command: &mut Command) -> io::Result<()> {
        let logs = self.path().join("processes");
        std::fs::create_dir_all(&logs)?;
        let index = self.processes.len();
        let label = safe_name(label);
        let stdout = File::create(logs.join(format!("{index}-{label}.stdout")))?;
        let stderr = File::create(logs.join(format!("{index}-{label}.stderr")))?;
        let mut arguments = vec![command.get_program().to_string_lossy().into_owned()];
        arguments.extend(command.get_args().map(|a| a.to_string_lossy().into_owned()));
        let binary = PathBuf::from(command.get_program());
        let mut process = Process {
            label,
            command: arguments,
            binary,
            child: None,
            spawn_error: None,
            started: Instant::now(),
        };
        match command
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
        {
            Ok(child) => {
                process.child = Some(self.children.len());
                self.children.push(child);
                self.processes.push(process);
                Ok(())
            }
            Err(error) => {
                process.spawn_error = Some(error.to_string());
                self.processes.push(process);
                Err(error)
            }
        }
    }
    pub fn children_mut(&mut self) -> &mut [Child] {
        &mut self.children
    }
    pub fn record(&self, name: &str, value: Value) {
        let mut context = self.context.borrow_mut();
        context.sequence += 1;
        let sequence = context.sequence;
        context.values.insert(name.to_owned(), value);
        context.order.insert(name.to_owned(), sequence);
    }
    pub fn preserve(&mut self, reason: &str) -> io::Result<PathBuf> {
        if let Some(path) = &self.captured {
            return Ok(path.clone());
        }
        if let Some(error) = &self.capture_error {
            return Err(io::Error::other(error.clone()));
        }
        let result = self.collect(reason);
        match &result {
            Ok(path) => {
                eprintln!(
                    "test failure diagnostics: {}",
                    path.join("report.json").display()
                );
                self.captured = Some(path.clone());
            }
            Err(error) => {
                eprintln!(
                    "could not preserve test failure diagnostics: {error}; original failure is unchanged"
                );
                self.capture_error = Some(error.to_string());
            }
        }
        result
    }
    pub fn check(&mut self, condition: bool, reason: &str) {
        if !condition {
            let artifact = self.preserve(reason);
            panic!("{reason}; diagnostics: {artifact:?}");
        }
    }
    fn collect(&mut self, reason: &str) -> io::Result<PathBuf> {
        // Snapshot live owned children BEFORE any cleanup or potentially expensive digest.
        let mut processes = vec![];
        let mut binaries = BTreeSet::new();
        for process in &self.processes {
            binaries.insert(process.binary.clone());
            let mut value = json!({"label":process.label, "command":process.command, "elapsed_ms":process.started.elapsed().as_millis(), "spawn_error":process.spawn_error});
            if let Some(index) = process.child {
                let child = &mut self.children[index];
                let pid = child.id();
                value["pid"] = json!(pid);
                match child.try_wait() {
                    Ok(Some(exit)) => {
                        value["state"] = json!("exited");
                        value["exit_code"] = json!(exit.code());
                        value["exit_status"] = json!(exit.to_string());
                    }
                    Ok(None) => {
                        value["state"] = json!("running");
                        value["os"] = process_snapshot(pid);
                    }
                    Err(error) => {
                        value["state"] = json!("unknown");
                        value["wait_error"] = json!(error.to_string());
                    }
                }
            } else {
                value["state"] = json!("spawn_failed");
            }
            processes.push(value);
        }
        let (context, context_sequence) = self
            .context
            .try_borrow()
            .map(|v| (json!(v.values), json!(v.order)))
            .unwrap_or_else(|_| {
                (
                    json!({"capture_error":"context was borrowed during panic"}),
                    Value::Null,
                )
            });
        // A handoff successor may not be a direct child. Inspect, but never signal,
        // a reported PID only when the last reply authenticates this private fixture.
        let reported_monitor = context.get("status.last_reply").and_then(|reply| {
            let config = reply["details"]["config_path"].as_str()?;
            if !Path::new(config).starts_with(self.path()) { return None; }
            let pid = u32::try_from(reply["status"]["monitor"]["pid"].as_u64()?).ok()?;
            Some(json!({"pid":pid,"os":process_snapshot(pid),"ownership":"fixture config path verified; not a child handle; never killed by collector"}))
        });
        std::fs::create_dir_all(&self.artifacts)?;
        let name = safe_name(std::thread::current().name().unwrap_or("test"));
        let destination = tempfile::Builder::new()
            .prefix(&format!("{name}-"))
            .tempdir_in(&self.artifacts)?
            .keep();
        let mut copied = 0;
        let mut bytes = 0;
        let mut omissions = vec![];
        if let Err(error) = copy_evidence(
            self.path(),
            &destination.join("files"),
            0,
            &mut copied,
            &mut bytes,
            &mut omissions,
        ) {
            omissions.push(format!("fixture copy incomplete: {error}"));
        }
        let identities: Vec<_> = binaries.into_iter().map(|path| {
            let metadata = std::fs::metadata(&path);
            let hash = herdr_idle_inhibitor::runtime::cache::fingerprint(&path);
            json!({"path":path,"bytes":metadata.as_ref().map(|v|v.len()).ok(),"modified_unix_ms":metadata.ok().and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d|d.as_millis()),"sha256":hash.as_ref().ok(),"hash_error":hash.err().map(|e|e.to_string())})
        }).collect();
        let report = json!({
            "schema_version":1,"test":name,"reason":reason,"fixture":self.path(),
            "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
            "ci_commit":std::env::var("GITHUB_SHA").ok(),"tokio_worker_threads":std::env::var("TOKIO_WORKER_THREADS").ok(),
            "context":context,"context_sequence":context_sequence,"processes":processes,"reported_monitor":reported_monitor,"binaries":identities,
            "copied_files":copied,"copied_bytes":bytes,"omissions":omissions,
            "notes":["Process observations precede child cleanup. No additional Herdr or monitor query was made.","CPU counters and file positions are observations, not a proven root-cause classification.","Only bounded fixture text/log files are copied; binaries, symlinks, sockets and full environment are excluded."]
        });
        std::fs::write(
            destination.join("report.json"),
            serde_json::to_vec_pretty(&report).map_err(io::Error::other)?,
        )?;
        Ok(destination)
    }
}
impl Drop for Diagnostics {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let _ = self.preserve("test panicked; see original assertion in test output");
        }
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn safe_name(name: &str) -> String {
    name.chars()
        .take(80)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
fn copy_evidence(
    source: &Path,
    destination: &Path,
    depth: usize,
    copied: &mut usize,
    bytes: &mut usize,
    omissions: &mut Vec<String>,
) -> io::Result<()> {
    if depth > 8 || *copied >= 128 || *bytes >= 8 * 1024 * 1024 {
        omissions.push(format!("capture limit: {}", source.display()));
        return Ok(());
    }
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                omissions.push(format!("directory entry unavailable: {error}"));
                continue;
            }
        };
        let path = entry.path();
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(error) => {
                omissions.push(format!(
                    "file type unavailable: {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let target = destination.join(entry.file_name());
        if kind.is_symlink() {
            omissions.push(format!("symlink skipped: {}", path.display()));
            continue;
        }
        if kind.is_dir() {
            if entry.file_name() != "bin"
                && entry.file_name() != "target"
                && let Err(error) =
                    copy_evidence(&path, &target, depth + 1, copied, bytes, omissions)
            {
                omissions.push(format!(
                    "directory copy incomplete: {}: {error}",
                    path.display()
                ));
            }
            continue;
        }
        if !kind.is_file()
            || !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("json" | "toml" | "log" | "stdout" | "stderr" | "plugins")
            )
        {
            continue;
        }
        if *copied >= 128 || *bytes >= 8 * 1024 * 1024 {
            omissions.push("capture file/byte limit reached".into());
            break;
        }
        // Tail logs when bounded; mark truncation rather than silently pretending completeness.
        let limit = (512 * 1024).min(8 * 1024 * 1024 - *bytes);
        let result = (|| -> io::Result<usize> {
            let mut file = File::open(&path)?;
            let length = file.metadata()?.len();
            if length > limit as u64 {
                file.seek(SeekFrom::End(-(limit as i64)))?;
                omissions.push(format!(
                    "truncated tail: {} ({length} bytes)",
                    path.display()
                ));
            }
            let mut data = vec![];
            file.take(limit as u64).read_to_end(&mut data)?;
            std::fs::write(target, data.as_slice())?;
            Ok(data.len())
        })();
        match result {
            Ok(length) => {
                *bytes += length;
                *copied += 1;
            }
            Err(error) => omissions.push(format!(
                "file copy unavailable: {}: {error}",
                path.display()
            )),
        }
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn process_snapshot(pid: u32) -> Value {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let stat = std::fs::read_to_string(proc.join("stat"));
    let mut snapshot = json!({"available":stat.is_ok(),"stat_error":stat.as_ref().err().map(|e|e.to_string()),"exe":std::fs::read_link(proc.join("exe")).ok(),"status":std::fs::read_to_string(proc.join("status")).ok(),"io":std::fs::read_to_string(proc.join("io")).ok(),"wchan":std::fs::read_to_string(proc.join("wchan")).ok()});
    if let Ok(stat) = stat {
        let fields: Vec<_> = stat
            .rsplit_once(") ")
            .map(|(_, s)| s.split_whitespace().collect())
            .unwrap_or_default();
        snapshot["user_ticks"] = json!(fields.get(11).and_then(|v| v.parse::<u64>().ok()));
        snapshot["system_ticks"] = json!(fields.get(12).and_then(|v| v.parse::<u64>().ok()));
        snapshot["stat"] = json!(stat);
    }
    let fds: Vec<_> = std::fs::read_dir(proc.join("fd")).into_iter().flatten().filter_map(Result::ok).take(64).map(|entry|json!({"fd":entry.file_name(),"target":std::fs::read_link(entry.path()).ok(),"info":std::fs::read_to_string(proc.join("fdinfo").join(entry.file_name())).ok()})).collect();
    snapshot["fds"] = json!(fds);
    snapshot
}
#[cfg(not(target_os = "linux"))]
fn process_snapshot(_pid: u32) -> Value {
    json!({"available":false,"reason":"Detailed /proc CPU/file-descriptor snapshots are Linux-only; portable PID, child exit state, stdout/stderr and binary identity are recorded separately."})
}
