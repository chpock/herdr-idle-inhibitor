use crate::{
    herdr::{
        discovery::Registration,
        transport::{check_peer, read_frame},
    },
    status::{Issue, Status},
};
use interprocess::local_socket::{
    ListenerOptions,
    tokio::{Stream, prelude::*},
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: Operation,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Operation {
    GetStatus {
        #[serde(default)]
        details: bool,
    },
    RegisterAndRefresh {
        registration: Registration,
    },
    SetPaused {
        paused: bool,
    },
    ApplySettings {
        patch: SettingsPatch,
    },
    ReloadSettings,
    PrepareUpgrade,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    pub release_delay_secs: Option<u64>,
    pub linux_backend: Option<String>,
    pub hypridle_integration_confirmed: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub protocol_version: u32,
    pub request_id: String,
    pub status: serde_json::Value,
    pub error: Option<Issue>,
    pub details: Option<Details>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Details {
    pub config: crate::runtime::config::Config,
    pub config_path: String,
    pub servers: Vec<ServerRow>,
    #[serde(default)]
    pub runtime: Option<super::update::RuntimeInfo>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRow {
    pub endpoint: String,
    pub version: Option<String>,
    pub current: bool,
    pub age_ms: Option<u64>,
}
pub struct Incoming {
    pub request: Request,
    pub reply: tokio::sync::oneshot::Sender<Reply>,
}
#[derive(Debug)]
pub struct QueryError {
    pub code: &'static str,
    pub exit: u8,
}
impl QueryError {
    fn io(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe => Self {
                code: "monitor_unavailable",
                exit: 3,
            },
            std::io::ErrorKind::PermissionDenied => Self {
                code: "permission_denied",
                exit: 5,
            },
            std::io::ErrorKind::InvalidData => Self {
                code: "protocol_error",
                exit: 5,
            },
            _ => Self {
                code: "query_io_error",
                exit: 5,
            },
        }
    }
    fn protocol() -> Self {
        Self {
            code: "protocol_error",
            exit: 5,
        }
    }
}
pub async fn query(endpoint: &str, operation: Operation) -> Result<Reply, QueryError> {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        #[cfg(unix)]
        crate::runtime::singleton::check_socket(std::path::Path::new(endpoint))
            .map_err(QueryError::io)?;
        #[cfg(unix)]
        let name = crate::herdr::transport::local_name(endpoint).map_err(QueryError::io)?;
        #[cfg(windows)]
        let name = endpoint
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
            .map_err(QueryError::io)?;
        let mut s = Stream::connect(name).await.map_err(QueryError::io)?;
        check_peer(&s).map_err(QueryError::io)?;
        let request = Request {
            protocol_version: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            operation,
        };
        let mut frame = serde_json::to_vec(&request).map_err(|_| QueryError::protocol())?;
        frame.push(b'\n');
        s.write_all(&frame).await.map_err(QueryError::io)?;
        let bytes = read_frame(&mut tokio::io::BufReader::new(s), 1024 * 1024)
            .await
            .map_err(QueryError::io)?
            .ok_or(QueryError {
                code: "monitor_unavailable",
                exit: 3,
            })?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| QueryError::protocol())?;
        if value["protocol_version"] != 1 {
            return Err(QueryError {
                code: "incompatible_monitor",
                exit: 5,
            });
        }
        if value["request_id"] != request.request_id {
            return Err(QueryError::protocol());
        }
        Status::from_wire(value["status"].clone()).map_err(|_| QueryError::protocol())?;
        serde_json::from_value(value).map_err(|_| QueryError::protocol())
    })
    .await
    .unwrap_or(Err(QueryError {
        code: "query_timeout",
        exit: 4,
    }))
}
pub async fn listen(
    listener: &interprocess::local_socket::tokio::Listener,
    tx: tokio::sync::mpsc::Sender<Incoming>,
) -> std::io::Result<()> {
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
    loop {
        let s = listener.accept().await?;
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            drop(s);
            continue;
        };
        let tx = tx.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                check_peer(&s)?;
                let mut s = s;
                let frame = read_frame(&mut tokio::io::BufReader::new(&mut s), 1024 * 1024)
                    .await?
                    .ok_or_else(|| std::io::Error::other("empty request"))?;
                let req: Request = serde_json::from_slice(&frame)
                    .map_err(|_| std::io::Error::other("invalid request"))?;
                if req.protocol_version != 1
                    || req.request_id.is_empty()
                    || req.request_id.len() > 128
                {
                    return Err(std::io::Error::other("invalid protocol"));
                }
                let (reply, rx) = tokio::sync::oneshot::channel();
                tx.send(Incoming {
                    request: req,
                    reply,
                })
                .await
                .map_err(|_| std::io::Error::other("owner stopped"))?;
                let reply = rx
                    .await
                    .map_err(|_| std::io::Error::other("owner stopped"))?;
                let mut bytes = serde_json::to_vec(&reply)
                    .map_err(|_| std::io::Error::other("reply serialization"))?;
                if bytes.len() > 1024 * 1024 {
                    return Err(std::io::Error::other("reply limit exceeded"));
                }
                bytes.push(b'\n');
                s.write_all(&bytes).await
            })
            .await;
        });
    }
}
/// Isolated test listener only. Production listeners are created under singleton ownership.
pub fn test_listener(
    endpoint: &str,
) -> std::io::Result<interprocess::local_socket::tokio::Listener> {
    #[cfg(unix)]
    let name = crate::herdr::transport::local_name(endpoint)?;
    #[cfg(windows)]
    let name = endpoint.to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
    ListenerOptions::new().name(name).create_tokio()
}
