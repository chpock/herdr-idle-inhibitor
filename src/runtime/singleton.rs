use super::paths::Paths;
use interprocess::local_socket::{ListenerOptions, tokio::Listener};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub fn private_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        match std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
        {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "unsafe application directory",
            ));
        }
    }
    #[cfg(windows)]
    {
        std::fs::create_dir_all(path)?;
        super::windows_security::protect_path(path)?;
    }
    Ok(())
}
#[cfg(unix)]
pub fn check_socket(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let m = std::fs::symlink_metadata(path)?;
    if !m.file_type().is_socket() || m.uid() != unsafe { libc::geteuid() } {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "unsafe socket owner/type",
        ));
    }
    Ok(())
}
pub struct Owner {
    pub listener: Listener,
    _lock: File,
    #[cfg(unix)]
    paths: Paths,
}
impl Owner {
    pub fn claim(paths: Paths) -> anyhow::Result<Option<Self>> {
        private_dir(&paths.runtime)?;
        let path = paths.runtime.join("owner.lock");
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let lock = options.open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let m = lock.metadata()?;
            anyhow::ensure!(
                m.is_file() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
                "unsafe lock file"
            );
        }
        #[cfg(windows)]
        super::windows_security::protect_path(&path)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        #[cfg(unix)]
        {
            match std::fs::symlink_metadata(&paths.endpoint) {
                Ok(_) => {
                    check_socket(Path::new(&paths.endpoint))?;
                    std::fs::remove_file(&paths.endpoint)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        #[cfg(unix)]
        let options = ListenerOptions::new()
            .name(crate::herdr::transport::local_name(&paths.endpoint)?)
            .reclaim_name(false);
        #[cfg(windows)]
        let options = {
            use interprocess::local_socket::ToNsName;
            use interprocess::os::windows::local_socket::ListenerOptionsExt;
            ListenerOptions::new()
                .name(
                    paths
                        .endpoint
                        .as_str()
                        .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?,
                )
                .security_descriptor(super::windows_security::descriptor()?)
        };
        let listener = options.create_tokio()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&paths.endpoint, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Some(Self {
            listener,
            _lock: lock,
            #[cfg(unix)]
            paths,
        }))
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if check_socket(Path::new(&self.paths.endpoint)).is_ok() {
                let _ = std::fs::remove_file(&self.paths.endpoint);
            }
        }
    }
}

/// Stable coordination inode; never unlink a live lock to force ownership.
pub(crate) fn coordination_file(
    directory: &Path,
    name: &str,
    create: bool,
) -> anyhow::Result<File> {
    if create {
        private_dir(directory)?;
    }
    let path = directory.join(name);
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        anyhow::ensure!(
            m.is_file() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
            "unsafe coordination file"
        );
    }
    #[cfg(windows)]
    if create {
        super::windows_security::protect_path(&path)?;
    }
    Ok(file)
}
