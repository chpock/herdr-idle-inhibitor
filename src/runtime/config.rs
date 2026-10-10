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
const CHECK_INTERVAL_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    modified: std::time::SystemTime,
    len: u64,
}
impl FileStamp {
    fn of(metadata: std::fs::Metadata) -> std::io::Result<Self> {
        Ok(Self {
            modified: metadata.modified()?,
            len: metadata.len(),
        })
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct IoCounts {
    pub metadata: u64,
    pub reads: u64,
    pub parses: u64,
    pub writes: u64,
}

pub struct ConfigStore {
    pub path: PathBuf,
    pub config: Config,
    pub valid: bool,
    pub pause_persisted: bool,
    pub error: Option<String>,
    disk: Option<Vec<u8>>,
    seen: Option<FileStamp>,
    retry_read: bool,
    check_due: u64,
    #[cfg(test)]
    pub(crate) io: IoCounts,
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
            seen: None,
            retry_read: false,
            check_due: 0,
            #[cfg(test)]
            io: IoCounts::default(),
        };
        let _ = s.refresh(false);
        s
    }
    /// Called only by the monitor's timer, never by a status query.
    pub fn poll(&mut self, now: u64) -> anyhow::Result<bool> {
        if now < self.check_due {
            return Ok(false);
        }
        self.check_due = now.saturating_add(CHECK_INTERVAL_MS);
        self.refresh(false)
    }
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.refresh(true).map(|_| ())
    }
    fn stamp(&mut self) -> std::io::Result<Option<FileStamp>> {
        #[cfg(test)]
        {
            self.io.metadata += 1;
        }
        match std::fs::metadata(&self.path) {
            Ok(metadata) => FileStamp::of(metadata).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn read(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        #[cfg(test)]
        {
            self.io.reads += 1;
        }
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn refresh(&mut self, force: bool) -> anyhow::Result<bool> {
        let result: anyhow::Result<bool> = (|| {
            let stamp = self.stamp().inspect_err(|_| self.retry_read = true)?;
            let Some(stamp) = stamp else {
                // Deletion is not a request to reset preferences or clear Pause.
                self.write(&self.config.clone(), true)?;
                self.valid = true;
                self.pause_persisted = true;
                self.error = None;
                return Ok(true);
            };
            if !force && !self.retry_read && self.seen.as_ref() == Some(&stamp) {
                return Ok(false);
            }
            self.seen = Some(stamp);
            self.retry_read = true;
            let bytes = self
                .read()?
                .ok_or_else(|| anyhow::anyhow!("configuration disappeared while reading"))?;
            self.retry_read = false;
            #[cfg(test)]
            {
                self.io.parses += 1;
            }
            let text = std::str::from_utf8(&bytes)?;
            let mut c = Config::parse(text).map_err(|error| {
                let Some(toml_error) = error.downcast_ref::<toml::de::Error>() else {
                    // Validation errors are fixed messages, not file contents.
                    return error;
                };
                let reason = if toml_error.message().starts_with("unknown field") {
                    "Unknown configuration field"
                } else if toml_error.message().starts_with("invalid type") {
                    "Wrong configuration type"
                } else {
                    "Invalid TOML or configuration fields"
                };
                if let Some(span) = toml_error.span() {
                    let prefix = text.get(..span.start).unwrap_or("");
                    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
                    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                    anyhow::anyhow!("{reason} at line {line}, column {column}")
                } else {
                    anyhow::anyhow!("{reason}")
                }
            })?;
            // A failed Pause save or update handover must not be undone by a
            // later automatic read of the old on-disk Pause value.
            let persisted = self.pause_persisted || c.paused == self.config.paused;
            if !self.pause_persisted {
                c.paused = self.config.paused;
            }
            let changed = !self.valid || self.config != c;
            self.config = c;
            self.disk = Some(bytes);
            self.valid = true;
            self.pause_persisted = persisted;
            self.error = None;
            Ok(changed)
        })();
        if let Err(error) = &result {
            self.error = Some(error.to_string());
        }
        result
    }
    fn save(&mut self, c: &Config) -> anyhow::Result<()> {
        c.validate()?;
        let current = self.read()?;
        anyhow::ensure!(
            current == self.disk,
            "config changed externally; wait for automatic loading before saving"
        );
        self.write(c, current.is_none())
    }
    fn write(&mut self, c: &Config, missing: bool) -> anyhow::Result<()> {
        c.validate()?;
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
        #[cfg(test)]
        {
            self.io.writes += 1;
        }
        tmp.write_all(&bytes)?;
        tmp.as_file().sync_all()?;
        if missing {
            // Do not overwrite a file recreated by an editor during this check.
            tmp.persist_noclobber(&self.path)?;
        } else {
            tmp.persist(&self.path)?;
        }
        self.disk = Some(bytes);
        self.retry_read = true;
        // persist's returned handle has closed; in particular, Windows can now
        // publish the completed write's modification time.
        self.seen = self.stamp()?;
        self.retry_read = false;
        Ok(())
    }
    pub fn apply(&mut self, c: Config) -> anyhow::Result<()> {
        let result: anyhow::Result<()> = (|| {
            anyhow::ensure!(
                self.valid,
                "correct invalid config; changes load automatically"
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        time::{Duration, UNIX_EPOCH},
    };

    fn edit(path: &std::path::Path, text: &str, time: u64) {
        std::fs::write(path, text).unwrap();
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000 + time)),
            )
            .unwrap();
    }

    #[test]
    fn unchanged_config_costs_one_metadata_check_per_two_seconds_and_no_content_io() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::open(dir.path().join("config.toml"));
        assert_eq!(
            store.io,
            IoCounts {
                metadata: 2,
                writes: 1,
                ..Default::default()
            }
        );
        assert!(!store.poll(0).unwrap());
        let baseline = store.io;
        for now in 1..2000 {
            assert!(!store.poll(now).unwrap());
        }
        assert_eq!(
            store.io, baseline,
            "100ms ticks must not stat or read before the deadline"
        );
        for now in [2000, 4000, 6000] {
            assert!(!store.poll(now).unwrap());
        }
        assert_eq!(
            store.io,
            IoCounts {
                metadata: baseline.metadata + 3,
                ..baseline
            }
        );
        let mut config = store.config.clone();
        config.release_delay_secs = 7;
        store.apply(config).unwrap();
        let saved = store.io;
        assert!(!store.poll(8000).unwrap());
        assert_eq!(
            store.io,
            IoCounts {
                metadata: saved.metadata + 1,
                ..saved
            },
            "own saves must update the observed stamp"
        );
        println!(
            "config idle counters: {baseline:?} -> {:?}; own save: {saved:?}",
            store.io
        );
    }

    #[test]
    fn changed_mtime_size_and_editor_replacement_are_loaded_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        edit(&path, "release_delay_secs = 5\n", 1);
        let mut store = ConfigStore::open(path.clone());
        let baseline = store.io;
        edit(&path, "release_delay_secs = 7\n", 2);
        assert!(store.poll(0).unwrap());
        assert_eq!(store.config.release_delay_secs, 7);
        assert_eq!(store.io.parses, baseline.parses + 1);
        assert_eq!(store.io.reads, baseline.reads + 1);
        // Size detects a changed file even when its modification time is preserved.
        edit(&path, "release_delay_secs = 12\n", 2);
        assert!(store.poll(2000).unwrap());
        assert_eq!(store.config.release_delay_secs, 12);
        let replacement = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
        edit(replacement.path(), "release_delay_secs = 3\n", 3);
        replacement.persist(&path).unwrap();
        assert!(store.poll(4000).unwrap());
        assert_eq!(store.config.release_delay_secs, 3);
        assert!(!store.poll(6000).unwrap());
        assert_eq!(store.io.reads, baseline.reads + 3);
        assert_eq!(store.io.parses, baseline.parses + 3);
        assert_eq!(store.io.writes, baseline.writes);
    }

    #[test]
    fn invalid_edits_are_not_reparsed_or_overwritten_and_startup_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        edit(&path, "release_delay_secs = 99\n", 1);
        let mut store = ConfigStore::open(path.clone());
        assert!(!store.valid);
        assert!(store.error.is_some());
        let failed = store.io;
        for now in [0, 2000, 4000] {
            assert!(!store.poll(now).unwrap());
        }
        assert_eq!(store.io.reads, failed.reads);
        assert_eq!(store.io.parses, failed.parses);
        assert_eq!(store.io.writes, 0);
        edit(&path, "release_delay_secs = 7\n", 2);
        assert!(store.poll(6000).unwrap());
        assert!(store.valid);
        assert!(store.error.is_none());
        let good = store.config.clone();
        edit(&path, "[broken\n", 3);
        assert!(store.poll(8000).is_err());
        let failed = store.io;
        for now in [10_000, 12_000] {
            assert!(!store.poll(now).unwrap());
        }
        assert!(store.error.is_some());
        assert_eq!(store.config, good);
        assert!(store.valid);
        assert_eq!(store.io.reads, failed.reads);
        assert_eq!(store.io.parses, failed.parses);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[broken\n");
        edit(&path, "release_delay_secs = 7\n", 4);
        assert!(
            !store.poll(14_000).unwrap(),
            "repair with identical settings clears errors without changing preferences"
        );
        assert!(store.error.is_none());
    }

    #[test]
    fn deletion_restores_current_preferences_and_unsaved_pause_without_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        let mut store = ConfigStore::open(path.clone());
        let mut config = store.config.clone();
        config.release_delay_secs = 7;
        config
            .additional_endpoints
            .push(dir.path().join("extra.sock").to_string_lossy().into_owned());
        config.linux.backend = "kde".into();
        store.apply(config).unwrap();
        store.inherit_pause(true);
        assert!(!store.pause_persisted);
        let expected = store.config.clone();
        let before = store.io;
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        assert!(store.poll(0).unwrap());
        assert_eq!(store.config, expected);
        assert!(store.pause_persisted);
        assert!(store.valid);
        assert!(store.error.is_none());
        assert_eq!(
            Config::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            expected
        );
        assert_eq!(store.io.reads, before.reads);
        assert_eq!(store.io.writes, before.writes + 1);
        let restored = store.io;
        assert!(!store.poll(2000).unwrap());
        assert_eq!(store.io.reads, restored.reads);
        assert_eq!(store.io.writes, restored.writes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn failed_creation_retries_at_the_interval_and_invalid_utf8_is_not_reparsed() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("nested");
        let path = parent.join("config.toml");
        std::fs::write(&parent, b"parent is a file").unwrap();
        let mut store = ConfigStore::open(path.clone());
        assert!(!store.valid);
        assert!(store.error.is_some());
        assert!(store.poll(0).is_err());
        let failed = store.io;
        assert!(!store.poll(1).unwrap());
        assert_eq!(store.io, failed);
        std::fs::remove_file(&parent).unwrap();
        assert!(store.poll(2000).unwrap());
        assert!(store.valid);
        assert!(store.error.is_none());
        assert_eq!(
            Config::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            Config::default()
        );
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(store.poll(4000).is_err());
        let failed = store.io;
        assert!(!store.poll(6000).unwrap());
        assert_eq!(store.io.reads, failed.reads);
        assert_eq!(store.io.parses, failed.parses);
        assert!(store.valid);
        assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe]);
    }

    #[test]
    fn read_failures_retry_but_never_clear_an_unsaved_pause() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = ConfigStore::open(path.clone());
        edit(&path, "invalid", 1);
        assert!(store.set_paused(true).is_err());
        assert!(!store.pause_persisted);
        edit(&path, "paused = false\nrelease_delay_secs = 7\n", 2);
        assert!(store.poll(0).unwrap());
        assert!(
            store.config.paused,
            "automatic read must not undo a failed Pause save"
        );
        assert!(!store.pause_persisted);
        assert_eq!(store.config.release_delay_secs, 7);
        // Read failure: a directory is not valid TOML, and must not be overwritten.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(store.poll(2000).is_err());
        let failed = store.io;
        assert!(store.poll(4000).is_err());
        assert_eq!(store.io.reads, failed.reads + 1);
        assert!(store.config.paused);
        std::fs::remove_dir(&path).unwrap();
        assert!(store.poll(6000).unwrap());
        assert!(store.pause_persisted);
        store.set_paused(false).unwrap();
        assert!(!store.config.paused);
    }
}
