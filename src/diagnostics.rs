use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};
pub fn open_log(root: &Path) -> std::io::Result<File> {
    crate::runtime::singleton::private_dir(root)?;
    let path = root.join("monitor.log");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(std::io::Error::other("log symlink rejected"));
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= 1024 * 1024) {
        let previous = root.join("monitor.log.1");
        match std::fs::remove_file(&previous) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        std::fs::rename(&path, previous)?;
    }
    let mut o = OpenOptions::new();
    o.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    o.open(path)
}
pub struct Logger {
    root: std::path::PathBuf,
    file: File,
}
impl Logger {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        Ok(Self {
            root: root.into(),
            file: open_log(root)?,
        })
    }
    pub fn transition(&mut self, code: &str) {
        if self.file.metadata().is_ok_and(|m| m.len() >= 1024 * 1024)
            && let Ok(f) = open_log(&self.root)
        {
            self.file = f;
        }
        let _ = writeln!(self.file, "{} {}", crate::clock::unix_ms(), code);
    }
}
pub fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && *c != '\u{7f}')
        .take(512)
        .collect()
}
