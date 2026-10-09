use herdr_idle_inhibitor::runtime::{
    ipc::{self, Operation},
    paths::Paths,
    singleton::Owner,
};
#[tokio::test]
async fn lock_inode_survives_and_loser_cannot_remove_socket() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = dir
        .path()
        .join("control.sock")
        .to_string_lossy()
        .into_owned();
    let paths = Paths {
        config: dir.path().join("config.toml"),
        state: dir.path().join("logs"),
        runtime: dir.path().join("runtime"),
        endpoint,
    };
    let owner = Owner::claim(paths.clone()).unwrap().unwrap();
    assert!(Owner::claim(paths.clone()).unwrap().is_none());
    let lock = std::fs::metadata(paths.runtime.join("owner.lock")).unwrap();
    drop(owner);
    assert!(paths.runtime.join("owner.lock").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            lock.ino(),
            std::fs::metadata(paths.runtime.join("owner.lock"))
                .unwrap()
                .ino()
        );
    }
    #[cfg(windows)]
    let _ = lock;
    drop(Owner::claim(paths).unwrap().unwrap());
}
#[cfg(unix)]
#[tokio::test]
async fn unsafe_socket_substitution_is_not_deleted() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("not-a-socket");
    let target = dir.path().join("target");
    std::fs::write(&target, "valuable").unwrap();
    symlink(&target, &socket).unwrap();
    let p = Paths {
        config: dir.path().join("config"),
        state: dir.path().join("logs"),
        runtime: dir.path().join("runtime"),
        endpoint: socket.to_string_lossy().into_owned(),
    };
    assert!(Owner::claim(p).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "valuable");
    assert!(
        std::fs::symlink_metadata(socket)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
#[tokio::test]
async fn malformed_and_empty_responses_are_not_empty_work() {
    use interprocess::local_socket::tokio::prelude::*;
    use tokio::io::AsyncWriteExt;
    let dir = tempfile::tempdir().unwrap();
    for (data, code) in [(b"".as_slice(), 3), (b"{}", 5), (b"{}\n", 5)] {
        let endpoint = dir
            .path()
            .join(format!("{}.sock", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        let listener = ipc::test_listener(&endpoint).unwrap();
        tokio::spawn(async move {
            let mut s = listener.accept().await.unwrap();
            let _ = herdr_idle_inhibitor::herdr::transport::read_frame(&mut s, 1024 * 1024).await;
            s.write_all(data).await.unwrap();
        });
        assert_eq!(
            ipc::query(&endpoint, Operation::GetStatus { details: false })
                .await
                .unwrap_err()
                .exit,
            code
        );
    }
}
