#[path = "support/tempdir.rs"]
mod fixtures;

#[test]
fn native_socket_fixture_identity_matches_its_canonical_parent() {
    let dir = fixtures::tempdir();
    #[cfg(unix)]
    {
        assert_eq!(std::fs::canonicalize(dir.path()).unwrap(), dir.path());
        let endpoint = dir.path().join("a.sock").to_string_lossy().into_owned();
        assert_eq!(
            herdr_idle_inhibitor::herdr::discovery::normalize_endpoint(&endpoint).unwrap(),
            endpoint
        );
        // macOS sockaddr_un.sun_path is 104 bytes including the trailing NUL.
        // Runtime's longest fixture endpoint is a UUID plus ".sock".
        let longest = dir.path().join(format!("{}.sock", uuid::Uuid::new_v4()));
        assert!(longest.as_os_str().as_encoded_bytes().len() < 104);
    }
    #[cfg(windows)]
    assert!(dir.path().is_absolute());
}
