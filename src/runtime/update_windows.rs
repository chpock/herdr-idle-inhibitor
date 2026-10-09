//! Windows checkout replacement must not retain mapped source images or CWD handles.
//! New popups cooperate via update.lock. The one-time compatibility fallback only
//! closes current-user processes whose queried image is an exact installed binary.
use std::path::{Path, PathBuf};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            QueryFullProcessImageNameW, TerminateProcess,
        },
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn equivalent(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}
fn clients(images: &[PathBuf]) -> anyhow::Result<Vec<Handle>> {
    // SAFETY: ToolHelp writes initialized, correctly sized Win32 structures.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    anyhow::ensure!(
        snapshot != INVALID_HANDLE_VALUE,
        "cannot inspect installed Windows clients"
    );
    let snapshot = Handle(snapshot);
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let own_sid = super::windows_security::sid(None)?;
    let mut matching = vec![];
    while more != 0 {
        if entry.th32ProcessID != std::process::id() {
            let raw = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                    0,
                    entry.th32ProcessID,
                )
            };
            if !raw.is_null() {
                let handle = Handle(raw);
                let mut path = vec![0u16; 32768];
                let mut length = path.len() as u32;
                if unsafe {
                    QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length)
                } != 0
                {
                    let path = PathBuf::from(String::from_utf16_lossy(&path[..length as usize]));
                    if images.iter().any(|image| equivalent(image, &path))
                        && super::windows_security::sid(Some(entry.th32ProcessID))
                            .is_ok_and(|sid| sid == own_sid)
                    {
                        // Keep the validated process handle, not a PID to reopen later.
                        matching.push(handle);
                    }
                }
            }
        }
        more = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Ok(matching)
}

pub async fn close_installed_clients(images: &[PathBuf]) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let matching = clients(images)?;
        if matching.is_empty() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            for handle in matching {
                // SAFETY: this exact handle was verified as our installed image
                // and current user. The already retired monitor released power first.
                anyhow::ensure!(
                    unsafe { TerminateProcess(handle.0, 0) } != 0,
                    "cannot close an installed Windows client"
                );
            }
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while !clients(images)?.is_empty() {
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "installed Windows clients did not exit"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    struct Children(Vec<Child>);
    impl Drop for Children {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    async fn ready(path: &Path) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while !path.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "native fixture startup deadline expired"
            );
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
    #[tokio::test]
    async fn replacement_closes_only_current_user_exact_installed_images_and_releases_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("installed");
        let cache = dir.path().join("private-cache");
        let unrelated = dir.path().join("unrelated");
        for path in [&root, &cache, &unrelated] {
            std::fs::create_dir(path).unwrap();
        }
        let image = root.join("herdr-idle-inhibitor.exe");
        assert!(
            Command::new("rustc")
                .args(["tests/support/hold_image.rs", "-o"])
                .arg(&image)
                .status()
                .unwrap()
                .success()
        );
        let mut children = Children(vec![]);
        for (index, folder) in [&root, &cache, &unrelated].into_iter().enumerate() {
            let executable = folder.join("herdr-idle-inhibitor.exe");
            if index != 0 {
                std::fs::copy(&image, &executable).unwrap();
            }
            let marker = dir.path().join(format!("ready-{index}"));
            children.0.push(
                Command::new(executable)
                    .arg(&marker)
                    .current_dir(folder)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            ready(&marker).await;
        }
        let moved = dir.path().join("replacement");
        assert!(
            std::fs::rename(&root, &moved).is_err(),
            "fixture must really lock the checkout directory"
        );
        close_installed_clients(&[image]).await.unwrap();
        assert!(children.0[0].try_wait().unwrap().is_some());
        assert!(
            children.0[1].try_wait().unwrap().is_none(),
            "cached monitor must not be terminated"
        );
        assert!(
            children.0[2].try_wait().unwrap().is_none(),
            "unrelated same-user image must not be terminated"
        );
        std::fs::rename(&root, &moved).unwrap();
    }
}
