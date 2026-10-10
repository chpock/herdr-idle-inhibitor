use super::{APP_ID, KdeRow, Kind, REASON};
use std::os::fd::AsRawFd;
use zbus::zvariant::OwnedFd;
use zbus::{Connection, Proxy};

fn spec(kind: &Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Hypridle => (
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        ),
        Kind::Gnome => (
            "org.gnome.SessionManager",
            "/org/gnome/SessionManager",
            "org.gnome.SessionManager",
        ),
        Kind::Kde => (
            "org.kde.Solid.PowerManagement",
            "/org/kde/Solid/PowerManagement/PolicyAgent",
            "org.kde.Solid.PowerManagement.PolicyAgent",
        ),
        _ => unreachable!(),
    }
}
async fn owner(conn: &Connection, service: &str) -> anyhow::Result<String> {
    let p = zbus::fdo::DBusProxy::new(conn).await?;
    Ok(p.get_name_owner(service.try_into()?).await?.to_string())
}
pub(super) async fn qualify(kind: &Kind) -> anyhow::Result<()> {
    let (bin, args, version) = match kind {
        Kind::Hypridle => ("hypridle", vec!["--version"], "0.1.8"),
        Kind::Gnome => ("gnome-session", vec!["--version"], "51.0"),
        Kind::Kde => {
            let bin = [
                "/usr/lib/org_kde_powerdevil",
                "/usr/libexec/org_kde_powerdevil",
            ]
            .into_iter()
            .find(|p| std::path::Path::new(p).is_file())
            .unwrap_or("org_kde_powerdevil");
            (bin, vec!["--version"], "6.7.5")
        }
        _ => unreachable!(),
    };
    let out = crate::herdr::discovery::bounded_command(
        std::path::Path::new(bin),
        &args,
        &Default::default(),
    )
    .await?;
    anyhow::ensure!(
        String::from_utf8(out)?
            .split_whitespace()
            .any(|v| v.trim_start_matches('v') == version),
        "unqualified desktop version"
    );
    Ok(())
}
pub struct Resource {
    conn: Option<Connection>,
    kind: Kind,
    service_owner: String,
    cookie: Option<u32>,
    fd: Option<OwnedFd>,
    kde_confirmed: bool,
}
impl Resource {
    pub async fn acquire(kind: &Kind) -> anyhow::Result<Self> {
        let conn = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            if *kind == Kind::Hypridle {
                Connection::system().await
            } else {
                Connection::session().await
            }
        })
        .await??;
        Self::acquire_on(kind, conn).await
    }
    async fn acquire_on(kind: &Kind, conn: Connection) -> anyhow::Result<Self> {
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let (service, path, iface) = spec(kind);
            let service_owner = owner(&conn, service).await?;
            let proxy = Proxy::new(&conn, service_owner.as_str(), path, iface).await?;
            let (cookie, fd) = match kind {
                Kind::Hypridle => {
                    let fd: OwnedFd = proxy
                        .call("Inhibit", &("idle", APP_ID, REASON, "block"))
                        .await?;
                    // SAFETY: descriptor is live and uniquely owned; CLOEXEC prevents child inheritance.
                    unsafe {
                        anyhow::ensure!(
                            libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) != -1,
                            "cannot protect inhibitor descriptor"
                        );
                    }
                    (None, Some(fd))
                }
                Kind::Gnome => (
                    Some(proxy.call("Inhibit", &(APP_ID, 0u32, REASON, 4u32)).await?),
                    None,
                ),
                Kind::Kde => {
                    let _: Vec<KdeRow> = proxy.get_property("RequestedInhibitions").await?;
                    let _: Vec<KdeRow> = proxy.get_property("ActiveInhibitions").await?;
                    (
                        Some(proxy.call("AddInhibition", &(1u32, APP_ID, REASON)).await?),
                        None,
                    )
                }
                _ => unreachable!(),
            };
            Ok::<_, anyhow::Error>((service_owner, cookie, fd))
        })
        .await;
        match result {
            Ok(Ok((service_owner, cookie, fd))) => Ok(Self {
                conn: Some(conn),
                kind: kind.clone(),
                service_owner,
                cookie,
                fd,
                kde_confirmed: false,
            }),
            other => {
                let _ = conn.close().await;
                match other {
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(e.into()),
                    _ => unreachable!(),
                }
            }
        }
    }
    pub async fn state(&mut self) -> anyhow::Result<String> {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let Some(conn) = self.conn.as_ref() else {
                return Ok("none".into());
            };
            let (service, path, iface) = spec(&self.kind);
            anyhow::ensure!(
                owner(conn, service).await? == self.service_owner,
                "native service restarted"
            );
            if self.kind == Kind::Kde {
                let p = Proxy::new(conn, self.service_owner.as_str(), path, iface).await?;
                let requested: Vec<KdeRow> = p.get_property("RequestedInhibitions").await?;
                let active: Vec<KdeRow> = p.get_property("ActiveInhibitions").await?;
                Ok(super::kde_state(&requested, &active, &mut self.kde_confirmed)?.into())
            } else {
                Ok("accepted".into())
            }
        })
        .await?
    }
    pub async fn release(&mut self) -> anyhow::Result<()> {
        self.fd.take();
        let Some(conn) = self.conn.take() else {
            return Ok(());
        };
        let result = if let Some(cookie) = self.cookie.take() {
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                let (_, path, iface) = spec(&self.kind);
                let p = Proxy::new(&conn, self.service_owner.as_str(), path, iface).await?;
                let method = if self.kind == Kind::Gnome {
                    "Uninhibit"
                } else {
                    "ReleaseInhibition"
                };
                p.call::<_, _, ()>(method, &cookie).await?;
                Ok::<_, anyhow::Error>(())
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|r| r)
        } else {
            Ok(())
        };
        // Even failed/timed-out cookie release closes the sole owning connection.
        conn.close().await?;
        result
    }
}
impl Drop for Resource {
    fn drop(&mut self) {
        self.fd.take();
        if let Some(conn) = self.conn.take()
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            handle.spawn(async move {
                let _ = conn.close().await;
            });
        }
    }
}
pub fn watch_power(tx: tokio::sync::mpsc::Sender<super::PowerEvent>) {
    tokio::spawn(async move {
        use futures_util::StreamExt;
        let Ok(conn) = Connection::system().await else {
            return;
        };
        let Ok(proxy) = Proxy::new(
            &conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await
        else {
            return;
        };
        let Ok(mut stream) = proxy.receive_signal("PrepareForSleep").await else {
            return;
        };
        while let Some(msg) = stream.next().await {
            if let Ok(sleep) = msg.body().deserialize::<bool>() {
                let _ = tx.try_send(if sleep {
                    super::PowerEvent::Suspend
                } else {
                    super::PowerEvent::Resume
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::BufRead,
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };
    struct Bus {
        child: Child,
        address: String,
    }
    impl Bus {
        fn start() -> Self {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("dbus-daemon required for isolated protocol tests");
            let mut address = String::new();
            std::io::BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            Self {
                child,
                address: address.trim().into(),
            }
        }
        async fn connect(&self) -> Connection {
            zbus::connection::Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .await
                .unwrap()
        }
    }
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    type LogindCalls = Arc<Mutex<Vec<(String, String, String, String)>>>;
    type GnomeCalls = Arc<Mutex<Vec<(String, u32, String, u32)>>>;
    struct Logind {
        calls: LogindCalls,
        peer: Arc<Mutex<Option<std::os::unix::net::UnixStream>>>,
    }
    #[zbus::interface(name = "org.freedesktop.login1.Manager")]
    impl Logind {
        fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> OwnedFd {
            self.calls
                .lock()
                .unwrap()
                .push((what.into(), who.into(), why.into(), mode.into()));
            let (a, b) = std::os::unix::net::UnixStream::pair().unwrap();
            b.set_nonblocking(true).unwrap();
            *self.peer.lock().unwrap() = Some(b);
            let fd: std::os::fd::OwnedFd = a.into();
            OwnedFd::from(fd)
        }
    }
    struct Gnome {
        calls: GnomeCalls,
        released: Arc<Mutex<Vec<u32>>>,
        delay: Arc<AtomicBool>,
        release_fail: Arc<AtomicBool>,
    }
    #[zbus::interface(name = "org.gnome.SessionManager")]
    impl Gnome {
        async fn inhibit(&self, app: &str, xid: u32, reason: &str, flags: u32) -> u32 {
            self.calls
                .lock()
                .unwrap()
                .push((app.into(), xid, reason.into(), flags));
            if self.delay.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
            7
        }
        fn uninhibit(&self, cookie: u32) -> zbus::fdo::Result<()> {
            self.released.lock().unwrap().push(cookie);
            if self.release_fail.load(Ordering::SeqCst) {
                Err(zbus::fdo::Error::Failed("injected failure".into()))
            } else {
                Ok(())
            }
        }
    }
    struct Kde {
        calls: Arc<Mutex<Vec<u32>>>,
        flags: Arc<std::sync::atomic::AtomicU32>,
    }
    #[zbus::interface(name = "org.kde.Solid.PowerManagement.PolicyAgent")]
    impl Kde {
        fn add_inhibition(&self, types: u32, app: &str, reason: &str) -> u32 {
            assert_eq!(app, APP_ID);
            assert_eq!(reason, REASON);
            self.calls.lock().unwrap().push(types);
            9
        }
        fn release_inhibition(&self, cookie: u32) {
            assert_eq!(cookie, 9);
            self.calls.lock().unwrap().push(0);
        }
        #[zbus(property)]
        fn requested_inhibitions(&self) -> Vec<KdeRow> {
            if self.flags.load(Ordering::SeqCst) == u32::MAX {
                vec![]
            } else {
                vec![(
                    "sleep".into(),
                    APP_ID.into(),
                    REASON.into(),
                    "block".into(),
                    self.flags.load(Ordering::SeqCst),
                )]
            }
        }
        #[zbus(property)]
        fn active_inhibitions(&self) -> Vec<KdeRow> {
            if self.flags.load(Ordering::SeqCst) == 3 {
                self.requested_inhibitions()
            } else {
                vec![]
            }
        }
    }
    #[tokio::test]
    async fn logind_requests_idle_only_and_fd_is_close_on_exec() {
        use std::io::Read;
        let bus = Bus::start();
        let service = bus.connect().await;
        let calls = Arc::new(Mutex::new(vec![]));
        let peer = Arc::new(Mutex::new(None));
        service
            .object_server()
            .at(
                "/org/freedesktop/login1",
                Logind {
                    calls: calls.clone(),
                    peer: peer.clone(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.login1")
            .await
            .unwrap();
        // First-run defaults must allow idle inhibition without listener confirmation.
        let kind =
            super::super::select(&crate::runtime::config::Config::default(), "Hyprland").unwrap();
        let mut r = Resource::acquire_on(&kind, bus.connect().await)
            .await
            .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![("idle".into(), APP_ID.into(), REASON.into(), "block".into())]
        );
        let fd = r.fd.as_ref().unwrap().as_raw_fd();
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let mut byte = [0u8; 1];
        assert_eq!(
            peer.lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .read(&mut byte)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        r.release().await.unwrap();
        // The isolated bus may briefly retain the transferred FD in its outgoing
        // message. Await observable EOF, not an assumed synchronous daemon drop.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let result = peer.lock().unwrap().as_mut().unwrap().read(&mut byte);
                match result {
                    Ok(0) => break,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                    other => panic!("unexpected released inhibitor FD state: {other:?}"),
                }
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn gnome_exact_flag_cookie_and_sole_connection_cleanup() {
        let bus = Bus::start();
        let service = bus.connect().await;
        let calls = Arc::new(Mutex::new(vec![]));
        let released = Arc::new(Mutex::new(vec![]));
        service
            .object_server()
            .at(
                "/org/gnome/SessionManager",
                Gnome {
                    calls: calls.clone(),
                    released: released.clone(),
                    delay: Arc::new(AtomicBool::new(false)),
                    release_fail: Default::default(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.gnome.SessionManager")
            .await
            .unwrap();
        let mut r = Resource::acquire_on(&Kind::Gnome, bus.connect().await)
            .await
            .unwrap();
        let name = r.conn.as_ref().unwrap().unique_name().unwrap().to_string();
        assert_eq!(r.state().await.unwrap(), "accepted");
        assert_eq!(
            *calls.lock().unwrap(),
            vec![(APP_ID.into(), 0, REASON.into(), 4)]
        );
        r.release().await.unwrap();
        assert_eq!(*released.lock().unwrap(), vec![7]);
        assert!(owner(&service, &name).await.is_err());
    }
    #[tokio::test]
    async fn kde_pending_active_suppressed_and_service_loss() {
        let bus = Bus::start();
        let service = bus.connect().await;
        let calls = Arc::new(Mutex::new(vec![]));
        let flags = Arc::new(std::sync::atomic::AtomicU32::new(u32::MAX));
        service
            .object_server()
            .at(
                "/org/kde/Solid/PowerManagement/PolicyAgent",
                Kde {
                    calls: calls.clone(),
                    flags: flags.clone(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.kde.Solid.PowerManagement")
            .await
            .unwrap();
        let mut r = Resource::acquire_on(&Kind::Kde, bus.connect().await)
            .await
            .unwrap();
        assert_eq!(r.state().await.unwrap(), "pending");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(r.state().await.unwrap(), "pending");
        assert_eq!(*calls.lock().unwrap(), vec![1]);
        flags.store(3, Ordering::SeqCst);
        assert_eq!(r.state().await.unwrap(), "accepted");
        flags.store(0, Ordering::SeqCst);
        assert_eq!(r.state().await.unwrap(), "suppressed");
        assert_eq!(*calls.lock().unwrap(), vec![1]);
        flags.store(u32::MAX, Ordering::SeqCst);
        assert!(r.state().await.is_err());
        service
            .release_name("org.kde.Solid.PowerManagement")
            .await
            .unwrap();
        assert!(r.state().await.is_err());
        r.release().await.unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![1, 0]);
    }
    #[tokio::test]
    async fn failed_cookie_release_still_closes_sole_connection() {
        let bus = Bus::start();
        let service = bus.connect().await;
        service
            .object_server()
            .at(
                "/org/gnome/SessionManager",
                Gnome {
                    calls: Default::default(),
                    released: Default::default(),
                    delay: Default::default(),
                    release_fail: Arc::new(AtomicBool::new(true)),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.gnome.SessionManager")
            .await
            .unwrap();
        let mut r = Resource::acquire_on(&Kind::Gnome, bus.connect().await)
            .await
            .unwrap();
        let name = r.conn.as_ref().unwrap().unique_name().unwrap().to_string();
        assert!(r.release().await.is_err());
        assert!(owner(&service, &name).await.is_err());
        assert_eq!(r.state().await.unwrap(), "none");
    }
    #[tokio::test]
    async fn replacement_owner_cannot_receive_old_cookie_release() {
        let bus = Bus::start();
        let old = bus.connect().await;
        let released = Arc::new(Mutex::new(vec![]));
        old.object_server()
            .at(
                "/org/gnome/SessionManager",
                Gnome {
                    calls: Default::default(),
                    released: released.clone(),
                    delay: Default::default(),
                    release_fail: Default::default(),
                },
            )
            .await
            .unwrap();
        old.request_name("org.gnome.SessionManager").await.unwrap();
        let mut r = Resource::acquire_on(&Kind::Gnome, bus.connect().await)
            .await
            .unwrap();
        old.release_name("org.gnome.SessionManager").await.unwrap();
        let replacement = bus.connect().await;
        let wrong = Arc::new(Mutex::new(vec![]));
        replacement
            .object_server()
            .at(
                "/org/gnome/SessionManager",
                Gnome {
                    calls: Default::default(),
                    released: wrong.clone(),
                    delay: Default::default(),
                    release_fail: Default::default(),
                },
            )
            .await
            .unwrap();
        replacement
            .request_name("org.gnome.SessionManager")
            .await
            .unwrap();
        assert!(r.state().await.is_err());
        r.release().await.unwrap();
        assert_eq!(*released.lock().unwrap(), vec![7]);
        assert!(wrong.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn lost_acquire_reply_closes_uncertain_owner_connection() {
        let bus = Bus::start();
        let service = bus.connect().await;
        service
            .object_server()
            .at(
                "/org/gnome/SessionManager",
                Gnome {
                    calls: Default::default(),
                    released: Default::default(),
                    delay: Arc::new(AtomicBool::new(true)),
                    release_fail: Default::default(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.gnome.SessionManager")
            .await
            .unwrap();
        let conn = bus.connect().await;
        let name = conn.unique_name().unwrap().to_string();
        assert!(Resource::acquire_on(&Kind::Gnome, conn).await.is_err());
        assert!(owner(&service, &name).await.is_err());
    }
}
