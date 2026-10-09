use herdr_idle_inhibitor::runtime::config::{Config, ConfigStore};
#[test]
fn strict_config_and_defaults() {
    assert_eq!(Config::default().release_delay_secs, 5);
    for text in [
        "schema_version = 2",
        "bogus = true",
        "release_delay_secs = 61",
        "release_delay_secs = -1",
        "additional_endpoints = ['ssh://host']",
        "[linux]\nbackend = 'sleep'",
    ] {
        assert!(Config::parse(text).is_err(), "accepted {text}");
    }
}
#[test]
fn invalid_reload_keeps_last_valid_and_external_edit_blocks_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut s = ConfigStore::open(path.clone());
    assert!(s.valid);
    let before = s.config.clone();
    std::fs::write(&path, "invalid").unwrap();
    assert!(s.reload().is_err());
    assert_eq!(s.config, before);
    assert!(s.set_paused(false).is_err());
    assert!(s.set_paused(true).is_err());
    assert!(s.config.paused);
    assert!(!s.pause_persisted);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid");
}
#[test]
fn persisted_pause_and_no_overwrite_of_invalid_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut s = ConfigStore::open(path.clone());
    s.set_paused(true).unwrap();
    assert!(ConfigStore::open(path.clone()).config.paused);
    std::fs::write(&path, "schema_version = 99").unwrap();
    let s = ConfigStore::open(path.clone());
    assert!(!s.valid);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "schema_version = 99"
    );
}

#[test]
fn rejected_reload_and_apply_are_retained_until_success_without_losing_valid_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut store = ConfigStore::open(path.clone());
    let original = std::fs::read_to_string(&path).unwrap();
    let good = store.config.clone();
    std::fs::write(&path, "invalid").unwrap();
    assert!(store.reload().is_err());
    assert!(
        store.error.is_some(),
        "rejected reload must remain observable"
    );
    assert_eq!(store.config, good);
    assert!(store.valid);
    std::fs::write(&path, &original).unwrap();
    store.reload().unwrap();
    assert!(store.error.is_none());
    std::fs::write(&path, format!("{original}\n# external edit\n")).unwrap();
    assert!(store.apply(good.clone()).is_err());
    assert!(
        store.error.is_some(),
        "rejected save must remain observable"
    );
    assert_eq!(store.config, good);
    store.reload().unwrap();
    assert!(store.error.is_none());
    let mut invalid = good.clone();
    invalid.release_delay_secs = 61;
    assert!(store.apply(invalid).is_err());
    assert!(store.error.is_some());
    assert_eq!(store.config, good);
    store.apply(good).unwrap();
    assert!(store.error.is_none());
}
