use super::{
    ipc::{Operation, Reply, query},
    paths::Paths,
    singleton::private_dir,
};
use crate::herdr::discovery::Registration;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

pub async fn ensure(r: Registration) -> anyhow::Result<Reply> {
    r.validate()?;
    let paths = Paths::get()?;
    let Some(_activation) = super::update::activation_guard(&paths, &r).await? else {
        anyhow::bail!("Update in progress; monitoring will resume automatically");
    };
    let operation = Operation::RegisterAndRefresh { registration: r };
    match query(&paths.endpoint, operation.clone()).await {
        Ok(reply) => return checked(reply, &paths).await,
        Err(e) if e.exit == 3 => (),
        Err(e) => anyhow::bail!("monitor activation failed: {}", e.code),
    }
    private_dir(&paths.state)?;
    let log = crate::diagnostics::open_log(&paths.state)?;
    let source = std::env::current_exe()?;
    let image = super::cache::stage(&source, &paths.state)?;
    let mut command = Command::new(&image.path);
    command
        .current_dir(&paths.state)
        .env("HERDR_IDLE_INHIBITOR_SOURCE_EXE", &source);
    command
        .arg("_serve")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("HERDR_")
            && key != "HERDR_CONFIG_PATH"
            && key != "HERDR_IDLE_INHIBITOR_CONFIG"
            && key != "HERDR_IDLE_INHIBITOR_STATE"
            && key != "HERDR_IDLE_INHIBITOR_SOURCE_EXE"
        {
            command.env_remove(key);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe; no allocation or locks in the post-fork hook.
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
    let mut child = command.spawn()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match query(&paths.endpoint, operation.clone()).await {
            Ok(reply) => return checked(reply, &paths).await,
            Err(e) if e.exit == 3 => (),
            Err(e) => anyhow::bail!("monitor activation failed: {}", e.code),
        }
        if child.try_wait()?.is_some() {
            // A race loser can have exited while the winner becomes ready.
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("monitor startup failed");
            }
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "monitor startup deadline expired"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn checked(reply: Reply, paths: &Paths) -> anyhow::Result<Reply> {
    if let Some(e) = &reply.error {
        anyhow::bail!("{}: {}", e.code, e.message);
    }
    super::update::replay_pending(paths).await?;
    Ok(reply)
}
