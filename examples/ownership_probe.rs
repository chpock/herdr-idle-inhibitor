use herdr_idle_inhibitor::backend::{Kind, NativeBackend, PowerBackend};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--child") {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        return Ok(());
    }
    anyhow::ensure!(
        cfg!(target_os = "linux"),
        "This diagnostic is Linux-only; it does not test sleep effectiveness"
    );
    let backend = NativeBackend;
    backend
        .probe(&Kind::Hypridle)
        .await
        .map_err(|i| anyhow::anyhow!(i.message))?;
    let mut resource = backend.acquire(&Kind::Hypridle).await?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--child")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    println!("owner={} child={}", std::process::id(), child.id());
    tokio::signal::ctrl_c().await?;
    resource.release().await?;
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}
