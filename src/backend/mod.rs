use crate::{runtime::config::Config, status::Issue};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub const APP_ID: &str = "herdr-idle-inhibitor";
pub const REASON: &str = "Herdr agents are working";
pub type KdeRow = (String, String, String, String, u32);
pub fn kde_state(
    requested: &[KdeRow],
    active: &[KdeRow],
    confirmed: &mut bool,
) -> anyhow::Result<&'static str> {
    let matching =
        |r: &&KdeRow| r.1 == APP_ID && r.2 == REASON && r.0.split(':').any(|w| w == "sleep");
    let Some(row) = requested.iter().find(matching) else {
        return if *confirmed {
            Err(anyhow::anyhow!("desktop no longer owns request"))
        } else {
            Ok("pending")
        };
    };
    *confirmed = true;
    if row.4 & 2 == 0 {
        return Ok("suppressed");
    }
    Ok(
        if active.iter().find(matching).is_some_and(|r| r.4 & 1 != 0) {
            "accepted"
        } else {
            "pending"
        },
    )
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Hypridle,
    Gnome,
    Kde,
    Macos,
    Windows,
}
impl Kind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hypridle => "hypridle-logind",
            Self::Gnome => "gnome-session",
            Self::Kde => "kde-powerdevil",
            Self::Macos => "macos-iokit",
            Self::Windows => "windows-power-request",
        }
    }
}
pub fn select(c: &Config, desktop: &str) -> Result<Kind, Issue> {
    #[cfg(target_os = "linux")]
    {
        let selected = if c.linux.backend == "auto" {
            let d = desktop.to_ascii_lowercase();
            let matches = [
                (d.contains("hyprland"), "hypridle"),
                (d.contains("gnome"), "gnome"),
                (d.contains("kde") || d.contains("plasma"), "kde"),
            ];
            let names: Vec<_> = matches
                .iter()
                .filter(|(yes, _)| *yes)
                .map(|(_, n)| *n)
                .collect();
            if names.len() != 1 {
                return Err(Issue::new(
                    "unsupported_profile",
                    "Select one supported desktop backend in Settings",
                ));
            }
            names[0]
        } else {
            &c.linux.backend
        };
        match selected {
            "hypridle" => Ok(Kind::Hypridle),
            "gnome" => Ok(Kind::Gnome),
            "kde" => Ok(Kind::Kde),
            _ => Err(Issue::new("unsupported_profile", "Unsupported desktop")),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (c, desktop);
        #[cfg(target_os = "macos")]
        return Ok(Kind::Macos);
        #[cfg(windows)]
        return Ok(Kind::Windows);
    }
}
pub enum Resource {
    #[cfg(target_os = "linux")]
    Linux(linux::Resource),
    #[cfg(target_os = "macos")]
    Macos(macos::Resource),
    #[cfg(windows)]
    Windows(windows::Resource),
}
impl Resource {
    pub async fn acquire(kind: &Kind) -> anyhow::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self::Linux(linux::Resource::acquire(kind).await?))
        }
        #[cfg(target_os = "macos")]
        {
            anyhow::ensure!(*kind == Kind::Macos, "invalid backend");
            Ok(Self::Macos(macos::Resource::acquire()?))
        }
        #[cfg(windows)]
        {
            anyhow::ensure!(*kind == Kind::Windows, "invalid backend");
            Ok(Self::Windows(windows::Resource::acquire()?))
        }
    }
    pub async fn state(&mut self) -> anyhow::Result<String> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(r) => r.state().await,
            #[cfg(target_os = "macos")]
            Self::Macos(r) => Ok(r.state().into()),
            #[cfg(windows)]
            Self::Windows(r) => Ok(r.state().into()),
        }
    }
    pub async fn release(&mut self) -> anyhow::Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(r) => r.release().await,
            #[cfg(target_os = "macos")]
            Self::Macos(r) => r.release(),
            #[cfg(windows)]
            Self::Windows(r) => r.release(),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum PowerEvent {
    Suspend,
    Resume,
}
pub fn power_events() -> anyhow::Result<tokio::sync::mpsc::Receiver<PowerEvent>> {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    #[cfg(target_os = "linux")]
    linux::watch_power(tx);
    #[cfg(windows)]
    windows::watch_power(tx)?;
    #[cfg(target_os = "macos")]
    drop(tx);
    Ok(rx)
}

/// Small compile-time/application boundary, not a loadable extension protocol.
#[async_trait::async_trait]
pub trait OwnedRequest: Send {
    async fn state(&mut self) -> anyhow::Result<String>;
    async fn release(&mut self) -> anyhow::Result<()>;
}
#[async_trait::async_trait]
pub trait PowerBackend: Send + Sync {
    async fn probe(&self, _kind: &Kind) -> Result<(), Issue> {
        Ok(())
    }
    async fn acquire(&self, kind: &Kind) -> anyhow::Result<Box<dyn OwnedRequest>>;
}
pub struct NativeBackend;
#[async_trait::async_trait]
impl PowerBackend for NativeBackend {
    async fn probe(&self, kind: &Kind) -> Result<(), Issue> {
        #[cfg(target_os = "linux")]
        {
            linux::qualify(kind).await.map_err(|_|Issue::new("unsupported_profile","Require Hypridle 0.1.8, GNOME session 51.0, or PowerDevil 6.7.5; the selected profile/version could not be qualified"))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = kind;
            Ok(())
        }
    }
    async fn acquire(&self, kind: &Kind) -> anyhow::Result<Box<dyn OwnedRequest>> {
        Ok(Box::new(Resource::acquire(kind).await?))
    }
}
#[async_trait::async_trait]
impl OwnedRequest for Resource {
    async fn state(&mut self) -> anyhow::Result<String> {
        Resource::state(self).await
    }
    async fn release(&mut self) -> anyhow::Result<()> {
        Resource::release(self).await
    }
}
