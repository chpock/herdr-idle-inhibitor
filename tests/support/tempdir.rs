/// Socket fixtures need a short, canonical parent on Unix. In particular, macOS
/// TMPDIR can be both long and behind /var -> /private/var. Production correctly
/// normalizes endpoint identity; test events must use the same native identity.
/// This does not change production paths or endpoint normalization.
pub fn tempdir() -> tempfile::TempDir {
    #[cfg(unix)]
    {
        tempfile::tempdir_in(std::fs::canonicalize("/tmp").unwrap()).unwrap()
    }
    #[cfg(windows)]
    {
        tempfile::tempdir().unwrap()
    }
}
