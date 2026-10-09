use herdr_idle_inhibitor::backend::{Kind, NativeBackend, PowerBackend};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let name = std::env::args().nth(1).ok_or_else(|| {
        anyhow::anyhow!(
            "Pass hypridle, gnome, kde, macos or windows; transient request ownership probe only"
        )
    })?;
    let kind = match name.as_str() {
        "hypridle" => Kind::Hypridle,
        "gnome" => Kind::Gnome,
        "kde" => Kind::Kde,
        "macos" => Kind::Macos,
        "windows" => Kind::Windows,
        _ => anyhow::bail!("invalid profile"),
    };
    let backend = NativeBackend;
    backend
        .probe(&kind)
        .await
        .map_err(|i| anyhow::anyhow!(i.message))?;
    let mut resource = backend.acquire(&kind).await?;
    println!(
        "pid={} request_state={}",
        std::process::id(),
        resource.state().await?
    );
    if std::env::args().any(|arg| arg == "--hold") {
        tokio::signal::ctrl_c().await?;
    }
    resource.release().await?;
    println!("released");
    Ok(())
}
