use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, process::Stdio};
use tokio::io::AsyncReadExt;

pub const PLUGIN_ID: &str = "herdr-idle-inhibitor";
const ROOT_ENV: &[&str] = &[
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "HERDR_CONFIG_PATH",
];
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub endpoint: String,
    pub herdr_bin: PathBuf,
    pub plugin_root: PathBuf,
    pub env: BTreeMap<String, String>,
    pub desktop: String,
    pub bus: Option<String>,
}
impl Registration {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::for_root(
            std::env::var("HERDR_BIN_PATH")?.into(),
            std::env::var("HERDR_PLUGIN_ROOT")?.into(),
            std::env::var("HERDR_SOCKET_PATH")?,
        )
    }
    pub fn for_root(
        herdr_bin: PathBuf,
        plugin_root: PathBuf,
        endpoint: String,
    ) -> anyhow::Result<Self> {
        let r = Self {
            endpoint,
            herdr_bin,
            plugin_root,
            env: ROOT_ENV
                .iter()
                .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
                .collect(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        };
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::runtime::config::validate_endpoint(&self.endpoint)?;
        anyhow::ensure!(
            self.herdr_bin.is_absolute() && self.herdr_bin.is_file(),
            "Herdr binary must exist at an absolute path"
        );
        anyhow::ensure!(
            self.plugin_root.is_absolute(),
            "plugin root must be absolute"
        );
        anyhow::ensure!(
            self.env.keys().all(|k| ROOT_ENV.contains(&k.as_str())),
            "unexpected registration environment"
        );
        anyhow::ensure!(
            self.env.values().all(|s| s.len() < 32768),
            "oversized registration"
        );
        Ok(())
    }
    pub fn root_key(&self) -> String {
        format!("{}:{:?}", self.herdr_bin.display(), self.env)
    }
}
#[derive(Debug, Deserialize)]
pub struct Session {
    pub name: String,
    pub running: bool,
    pub socket_path: String,
}
#[derive(Deserialize)]
struct Sessions {
    sessions: Vec<Session>,
}
pub fn root_command(
    bin: &std::path::Path,
    args: &[&str],
    env: &BTreeMap<String, String>,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(bin);
    command
        .args(args)
        .env_remove("HERDR_SOCKET_PATH")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    for key in ROOT_ENV {
        command.env_remove(key);
    }
    command.envs(env);
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("HERDR_PLUGIN_") {
            command.env_remove(k);
        }
    }
    command
}
pub async fn bounded_command(
    bin: &std::path::Path,
    args: &[&str],
    env: &BTreeMap<String, String>,
) -> anyhow::Result<Vec<u8>> {
    let mut child = root_command(bin, args, env).spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let mut bytes = Vec::new();
        stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes).await?;
        anyhow::ensure!(bytes.len() <= 1024 * 1024, "command output too large");
        anyhow::ensure!(child.wait().await?.success(), "Herdr command failed");
        Ok::<_, anyhow::Error>(bytes)
    })
    .await;
    match result {
        Ok(Ok(v)) => Ok(v),
        other => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match other {
                Ok(Err(e)) => Err(e),
                Err(e) => Err(e.into()),
                _ => unreachable!(),
            }
        }
    }
}
pub async fn discover(r: &Registration) -> anyhow::Result<Vec<Session>> {
    let bytes = bounded_command(&r.herdr_bin, &["session", "list", "--json"], &r.env).await?;
    let mut s: Sessions = serde_json::from_slice(&bytes)?;
    for session in &mut s.sessions {
        crate::runtime::config::validate_endpoint(&session.socket_path)?;
        if session.running {
            session.socket_path = normalize_endpoint(&session.socket_path)?;
        }
    }
    Ok(s.sessions)
}

pub fn normalize_endpoint(endpoint: &str) -> anyhow::Result<String> {
    crate::runtime::config::validate_endpoint(endpoint)?;
    #[cfg(unix)]
    {
        let path = std::path::Path::new(endpoint);
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("endpoint parent missing"))?;
        match std::fs::canonicalize(parent) {
            Ok(parent) => {
                let file = path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("endpoint filename missing"))?;
                Ok(parent.join(file).to_string_lossy().into_owned())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(endpoint.into()),
            Err(e) => Err(e.into()),
        }
    }
    #[cfg(windows)]
    {
        Ok(endpoint.into())
    } // Herdr names its pipe from this exact string; never case-fold it.
}
