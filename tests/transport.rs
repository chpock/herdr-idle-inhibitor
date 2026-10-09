#[path = "support/tempdir.rs"]
mod fixtures;

use herdr_idle_inhibitor::herdr::{protocol, transport};
use tokio::io::AsyncWriteExt;
#[tokio::test]
async fn partial_oversized_and_non_utf8_frames_are_not_empty_observations() {
    for (bytes, limit) in [
        (b"{unfinished".as_slice(), 64),
        (b"0123456789\n".as_slice(), 4),
    ] {
        let (mut writer, mut reader) = tokio::io::duplex(64);
        writer.write_all(bytes).await.unwrap();
        drop(writer);
        assert!(transport::read_frame(&mut reader, limit).await.is_err());
    }
    let (mut writer, mut reader) = tokio::io::duplex(64);
    writer.write_all(b"\xff\n").await.unwrap();
    drop(writer);
    let frame = transport::read_frame(&mut reader, 64)
        .await
        .unwrap()
        .unwrap();
    assert!(protocol::parse_agents(&frame, "id").is_err());
    assert!(
        protocol::parse_agents(
            b"{\"id\":\"wrong\",\"result\":{\"type\":\"agent_list\",\"agents\":[]}}",
            "id"
        )
        .is_err()
    );
}
#[tokio::test]
async fn stalled_native_peer_is_bounded_by_request_deadline() {
    use interprocess::local_socket::tokio::prelude::*;
    let dir = fixtures::tempdir();
    let endpoint = dir.path().join("stall.sock").to_string_lossy().into_owned();
    let listener = herdr_idle_inhibitor::runtime::ipc::test_listener(&endpoint).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _hold) = (listener.accept().await.unwrap(), listener);
        transport::read_frame(&mut tokio::io::BufReader::new(&mut stream), 1024)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    });
    assert!(
        transport::call(&endpoint, "id", "agent.list", serde_json::json!({}))
            .await
            .is_err()
    );
    server.abort();
}
