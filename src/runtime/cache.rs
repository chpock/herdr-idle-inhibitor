//! Immutable executable copies keep long-lived processes out of Herdr's checkout.
use super::singleton::private_dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub const FILENAME: &str = if cfg!(windows) {
    "herdr-idle-inhibitor.exe"
} else {
    "herdr-idle-inhibitor"
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binary {
    pub path: PathBuf,
    pub digest: String,
}

pub fn installed(root: &Path) -> PathBuf {
    root.join("target/release").join(FILENAME)
}

pub fn fingerprint(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "executable is not a regular file"
    );
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn stage(source: &Path, state: &Path) -> anyhow::Result<Binary> {
    // Hash the copied bytes, not a separately opened path that could be replaced.
    private_dir(state)?;
    let bin = state.join("bin");
    private_dir(&bin)?;
    let mut copy = tempfile::NamedTempFile::new_in(&bin)?;
    let mut input = std::fs::File::open(source)?;
    anyhow::ensure!(
        input.metadata()?.is_file(),
        "executable is not a regular file"
    );
    std::io::copy(&mut input, copy.as_file_mut())?;
    let digest = fingerprint(copy.path())?;
    let directory = bin.join(&digest);
    private_dir(&directory)?;
    let path = directory.join(FILENAME);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        copy.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    super::windows_security::protect_path(copy.path())?;
    match copy.persist_noclobber(&path) {
        Ok(_) => (),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.error.into()),
    }
    let metadata = std::fs::symlink_metadata(&path)?;
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "unsafe cached executable type"
    );
    anyhow::ensure!(
        fingerprint(&path)? == digest,
        "cached executable identity mismatch"
    );
    Ok(Binary { path, digest })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    length: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed: (i64, i64),
    #[cfg(windows)]
    created: u64,
}
impl Stamp {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let m = std::fs::metadata(path)?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        #[cfg(windows)]
        use std::os::windows::fs::MetadataExt;
        Ok(Self {
            length: m.len(),
            modified: m.modified().ok(),
            #[cfg(unix)]
            device: m.dev(),
            #[cfg(unix)]
            inode: m.ino(),
            #[cfg(unix)]
            changed: (m.ctime(), m.ctime_nsec()),
            #[cfg(windows)]
            created: m.creation_time(),
        })
    }
}
