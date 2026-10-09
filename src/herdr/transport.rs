use interprocess::local_socket::{
    Name,
    tokio::{Stream, prelude::*},
};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let mut one = [0u8; 1];
    loop {
        if reader.read(&mut one).await? == 0 {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated frame",
                ))
            };
        }
        if one[0] == b'\n' {
            return Ok(Some(bytes));
        }
        if bytes.len() >= limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame limit exceeded",
            ));
        }
        bytes.push(one[0]);
    }
}
pub fn local_name(endpoint: &str) -> io::Result<Name<'_>> {
    #[cfg(unix)]
    {
        endpoint.to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }
    #[cfg(windows)]
    {
        endpoint.to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }
}
pub fn check_peer(stream: &Stream) -> io::Result<()> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::traits::StreamCommon;
        let uid = stream.peer_creds()?.euid();
        // SAFETY: geteuid has no pointer arguments.
        if uid != Some(unsafe { libc::geteuid() }) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "peer is not current user",
            ));
        }
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::traits::StreamCommon;
        let pid = stream.peer_creds()?.pid().ok_or_else(|| {
            io::Error::new(io::ErrorKind::PermissionDenied, "peer PID unavailable")
        })?;
        if crate::runtime::windows_security::sid(Some(pid))?
            != crate::runtime::windows_security::sid(None)?
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "peer is not current user",
            ));
        }
    }
    Ok(())
}
pub async fn call(
    endpoint: &str,
    id: &str,
    method: &str,
    params: serde_json::Value,
) -> anyhow::Result<Vec<u8>> {
    super::super::runtime::config::validate_endpoint(endpoint)?;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        #[cfg(unix)]
        crate::runtime::singleton::check_socket(std::path::Path::new(endpoint))?;
        let mut s = Stream::connect(local_name(endpoint)?).await?;
        check_peer(&s)?;
        let mut bytes =
            serde_json::to_vec(&serde_json::json!({"id":id,"method":method,"params":params}))?;
        bytes.push(b'\n');
        s.write_all(&bytes).await?;
        let frame = read_frame(&mut tokio::io::BufReader::new(s), 8 * 1024 * 1024)
            .await?
            .ok_or_else(|| anyhow::anyhow!("empty response"))?;
        Ok::<_, anyhow::Error>(frame)
    })
    .await?
}
