use std::path::PathBuf;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
    pub runtime: PathBuf,
    pub endpoint: String,
}
impl Paths {
    pub fn get() -> anyhow::Result<Self> {
        #[cfg(unix)]
        let runtime = std::fs::canonicalize("/tmp")?
            .join(format!("herdr-idle-inhibitor-{}", unsafe {
                libc::geteuid()
            }));
        #[cfg(target_os = "linux")]
        let (config, state) = {
            let home = PathBuf::from(
                std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME missing"))?,
            );
            (
                std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".config"))
                    .join("herdr-idle-inhibitor/config.toml"),
                std::env::var_os("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".local/state"))
                    .join("herdr-idle-inhibitor"),
            )
        };
        #[cfg(target_os = "macos")]
        let (config, state) = {
            let home = directories::BaseDirs::new()
                .ok_or_else(|| anyhow::anyhow!("home missing"))?
                .home_dir()
                .to_path_buf();
            (
                home.join("Library/Application Support/herdr-idle-inhibitor/config.toml"),
                home.join("Library/Logs/herdr-idle-inhibitor"),
            )
        };
        #[cfg(windows)]
        let (config, state, runtime) = {
            let base = super::windows_security::local_app_data()?.join("herdr-idle-inhibitor");
            (
                base.join("config.toml"),
                base.join("logs"),
                base.join("runtime"),
            )
        };
        #[cfg(unix)]
        let endpoint = runtime.join("control.sock").to_string_lossy().into_owned();
        #[cfg(windows)]
        let endpoint = format!(
            "herdr-idle-inhibitor-{}",
            super::windows_security::sid(None)?
        );
        let config = std::env::var_os("HERDR_IDLE_INHIBITOR_CONFIG")
            .map(PathBuf::from)
            .unwrap_or(config);
        let state = std::env::var_os("HERDR_IDLE_INHIBITOR_STATE")
            .map(PathBuf::from)
            .unwrap_or(state);
        anyhow::ensure!(
            config.is_absolute() && state.is_absolute(),
            "application paths must be absolute"
        );
        Ok(Self {
            config,
            state,
            runtime,
            endpoint,
        })
    }
}
