use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub paused: bool,
    pub release_delay_secs: u64,
    pub additional_endpoints: Vec<String>,
    pub linux: Linux,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Linux {
    pub backend: String,
    /// Read old configuration files without requiring a migration. No runtime effect.
    #[serde(skip_serializing)]
    pub hypridle_integration_confirmed: bool,
}
impl Default for Linux {
    fn default() -> Self {
        Self {
            backend: "auto".into(),
            hypridle_integration_confirmed: false,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            paused: false,
            release_delay_secs: 5,
            additional_endpoints: vec![],
            linux: Linux::default(),
        }
    }
}
impl Config {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let c: Self = toml::from_str(text)?;
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.release_delay_secs <= 60,
            "release delay must be 0–60 seconds"
        );
        anyhow::ensure!(
            matches!(
                self.linux.backend.as_str(),
                "auto" | "hypridle" | "gnome" | "kde"
            ),
            "invalid backend"
        );
        anyhow::ensure!(self.additional_endpoints.len() <= 128, "too many endpoints");
        for e in &self.additional_endpoints {
            validate_endpoint(e)?;
        }
        Ok(())
    }
}
pub fn validate_endpoint(e: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !e.contains('\0') && !e.contains("://"),
        "endpoint must be local"
    );
    anyhow::ensure!(
        std::path::Path::new(e).is_absolute(),
        "endpoint must be an absolute local path"
    );
    #[cfg(windows)]
    anyhow::ensure!(
        !e.starts_with("\\\\") && !e.starts_with("//"),
        "remote UNC endpoint rejected"
    );
    Ok(())
}
pub struct ConfigStore {
    pub path: PathBuf,
    pub config: Config,
    pub valid: bool,
    pub pause_persisted: bool,
    pub error: Option<String>,
    disk: Option<Vec<u8>>,
}
impl ConfigStore {
    pub fn open(path: PathBuf) -> Self {
        let mut s = Self {
            path,
            config: Config::default(),
            valid: false,
            pause_persisted: true,
            error: None,
            disk: None,
        };
        match std::fs::read(&s.path) {
            Ok(bytes) => {
                s.disk = Some(bytes);
                if let Err(e) = s.reload() {
                    s.error = Some(e.to_string());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                s.valid = true;
                if let Err(e) = s.save(&s.config.clone()) {
                    s.valid = false;
                    s.error = Some(e.to_string());
                }
            }
            Err(e) => s.error = Some(e.to_string()),
        }
        s
    }
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let result: anyhow::Result<()> = (|| {
            let bytes = std::fs::read(&self.path)?;
            let c = Config::parse(std::str::from_utf8(&bytes)?)?;
            self.config = c;
            self.disk = Some(bytes);
            self.valid = true;
            self.pause_persisted = true;
            self.error = None;
            Ok(())
        })();
        if let Err(error) = &result {
            self.error = Some(error.to_string());
        }
        result
    }
    fn save(&mut self, c: &Config) -> anyhow::Result<()> {
        c.validate()?;
        let current = match std::fs::read(&self.path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            current == self.disk,
            "config changed externally; Reload before saving"
        );
        let parent = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("config parent missing"))?;
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        let bytes = toml::to_string_pretty(c)?.into_bytes();
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        tmp.write_all(&bytes)?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path)?;
        self.disk = Some(bytes);
        Ok(())
    }
    pub fn apply(&mut self, c: Config) -> anyhow::Result<()> {
        let result: anyhow::Result<()> = (|| {
            anyhow::ensure!(self.valid, "correct invalid config and Reload first");
            self.save(&c)?;
            self.config = c;
            self.pause_persisted = true;
            self.error = None;
            Ok(())
        })();
        if let Err(error) = &result {
            self.error = Some(error.to_string());
        }
        result
    }
    /// Carry an explicitly unsaved Pause through an update without writing settings.
    pub fn inherit_pause(&mut self, paused: bool) {
        if self.config.paused != paused {
            self.config.paused = paused;
            self.pause_persisted = false;
        }
    }
    pub fn set_paused(&mut self, paused: bool) -> anyhow::Result<()> {
        let mut c = self.config.clone();
        c.paused = paused;
        if paused {
            self.config.paused = true;
        }
        match self.apply(c) {
            Ok(()) => Ok(()),
            Err(e) => {
                if paused {
                    self.pause_persisted = false;
                }
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }
}
