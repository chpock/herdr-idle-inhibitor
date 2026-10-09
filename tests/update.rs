#[path = "support/tempdir.rs"]
mod fixtures;

use herdr_idle_inhibitor::{
    herdr::protocol::{Plugin, PluginSource},
    runtime::{
        cache::{fingerprint, stage},
        update::{Publication, SourceTracker},
    },
};
use std::path::PathBuf;

#[test]
fn artifact_identity_detects_different_code_with_the_same_package_version() {
    let dir = fixtures::tempdir();
    let old = dir.path().join("old");
    let new = dir.path().join("new");
    std::fs::write(&old, b"version=0.1.0; implementation=old").unwrap();
    std::fs::write(&new, b"version=0.1.0; implementation=new").unwrap();
    assert_ne!(fingerprint(&old).unwrap(), fingerprint(&new).unwrap());
    assert_eq!(fingerprint(&old).unwrap(), fingerprint(&old).unwrap());
}

#[test]
fn cached_images_are_outside_the_installation_and_never_overwritten() {
    let dir = fixtures::tempdir();
    let installed = dir.path().join("installed/app");
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, b"old image").unwrap();
    let state = dir.path().join("state");
    let old = stage(&installed, &state).unwrap();
    assert!(old.path.starts_with(&state));
    assert!(!old.path.starts_with(installed.parent().unwrap()));
    assert_eq!(stage(&installed, &state).unwrap().path, old.path);
    std::fs::write(&installed, b"new image").unwrap();
    let new = stage(&installed, &state).unwrap();
    assert_ne!(new.path, old.path);
    assert_eq!(std::fs::read(&old.path).unwrap(), b"old image");
    assert_eq!(std::fs::read(&new.path).unwrap(), b"new image");
}

#[test]
fn concurrent_cache_writers_publish_one_matching_complete_image() {
    let dir = fixtures::tempdir();
    let image = dir.path().join("app");
    std::fs::write(&image, vec![42; 128 * 1024]).unwrap();
    let state = dir.path().join("state");
    let images: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| stage(&image, &state).unwrap()))
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    for cached in &images {
        assert_eq!(cached.path, images[0].path);
        assert_eq!(
            fingerprint(&cached.path).unwrap(),
            fingerprint(&image).unwrap()
        );
    }
}

#[test]
fn publication_requires_matching_files_and_committed_registry_not_just_a_new_path() {
    let dir = fixtures::tempdir();
    let root = dir.path().join("plugin");
    let image = root.join("target/release/app");
    std::fs::create_dir_all(image.parent().unwrap()).unwrap();
    std::fs::write(&image, b"old image").unwrap();
    let new = dir.path().join("candidate");
    std::fs::write(&new, b"new image").unwrap();
    let publication = Publication {
        executable: image.clone(),
        digest: fingerprint(&new).unwrap(),
        plugin_root: root.clone(),
        expected_commit: Some("new-commit".into()),
    };
    let mut plugin = Plugin {
        plugin_id: "herdr-idle-inhibitor".into(),
        plugin_root: root.to_string_lossy().into_owned(),
        enabled: true,
        source: Some(PluginSource {
            resolved_commit: Some("old-commit".into()),
        }),
    };
    assert!(!publication.ready(&plugin).unwrap());
    std::fs::copy(&new, &image).unwrap();
    assert!(
        !publication.ready(&plugin).unwrap(),
        "files are not an install commit"
    );
    plugin.source.as_mut().unwrap().resolved_commit = Some("new-commit".into());
    assert!(publication.ready(&plugin).unwrap());
    plugin.enabled = false;
    assert!(!publication.ready(&plugin).unwrap());
    plugin.enabled = true;
    plugin.plugin_root = dir.path().join("unrelated").to_string_lossy().into_owned();
    assert!(!publication.ready(&plugin).unwrap());
}

#[test]
fn watcher_does_not_oscillate_between_unchanged_installations() {
    let dir = fixtures::tempdir();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"version a").unwrap();
    std::fs::write(&b, b"older version b").unwrap();
    let loaded = fingerprint(&a).unwrap();
    let mut tracker = SourceTracker::new(loaded, a.clone());
    assert!(tracker.inspect("a", &a).unwrap().is_none());
    assert!(tracker.inspect("b", &b).unwrap().is_none());
    for _ in 0..4 {
        assert!(tracker.inspect("a", &a).unwrap().is_none());
        assert!(tracker.inspect("b", &b).unwrap().is_none());
    }
    std::fs::write(&b, b"updated version b").unwrap();
    let changed = tracker.inspect("b", &b).unwrap().unwrap();
    assert_eq!(changed.path, b);
    assert_eq!(changed.digest, fingerprint(&b).unwrap());
}

#[test]
fn watcher_detects_replacement_and_relink_without_version_or_event_changes() {
    let dir = fixtures::tempdir();
    let old = dir.path().join("old");
    let new = dir.path().join("new");
    std::fs::write(&old, b"old code 0.1.0").unwrap();
    std::fs::write(&new, b"new code 0.1.0").unwrap();
    let mut tracker = SourceTracker::new(fingerprint(&old).unwrap(), old.clone());
    assert!(tracker.inspect("root", &old).unwrap().is_none());
    let changed = tracker.inspect("root", &new).unwrap().unwrap();
    assert_eq!(changed.path, new);
    assert!(tracker.inspect("root", &new).unwrap().is_none());
    let replacement = dir.path().join("replacement");
    std::fs::write(&replacement, b"next code 0.1.0").unwrap();
    std::fs::rename(&replacement, &new).unwrap();
    assert!(tracker.inspect("root", &new).unwrap().is_some());
}

#[test]
fn missing_or_unchanged_artifacts_do_not_stop_a_working_monitor() {
    let dir = fixtures::tempdir();
    let image = dir.path().join("app");
    std::fs::write(&image, b"same image").unwrap();
    let mut tracker = SourceTracker::new(fingerprint(&image).unwrap(), image.clone());
    assert!(tracker.inspect("root", &image).unwrap().is_none());
    let missing = PathBuf::from(dir.path()).join("not-published");
    assert!(tracker.inspect("root", &missing).unwrap().is_none());
    assert!(tracker.inspect("root", &image).unwrap().is_none());
}

#[test]
fn unchanged_source_polls_use_metadata_cache_and_failed_replacement_can_retry() {
    let dir = fixtures::tempdir();
    let image = dir.path().join("app");
    std::fs::write(&image, b"old image").unwrap();
    let mut tracker = SourceTracker::new(fingerprint(&image).unwrap(), image.clone());
    assert!(tracker.inspect("root", &image).unwrap().is_none());
    for _ in 0..100 {
        assert!(tracker.inspect("root", &image).unwrap().is_none());
    }
    assert_eq!(
        tracker.fingerprints_read(),
        1,
        "unchanged polls must not rehash the executable"
    );
    std::fs::write(&image, b"new image").unwrap();
    assert!(tracker.inspect("root", &image).unwrap().is_some());
    assert_eq!(tracker.fingerprints_read(), 2);
    assert!(tracker.inspect("root", &image).unwrap().is_none());
    tracker.retry(&image);
    assert!(tracker.inspect("root", &image).unwrap().is_some());
    assert_eq!(tracker.fingerprints_read(), 3);
}

#[test]
fn rejected_artifact_is_not_automatically_retried_but_new_contents_are() {
    let dir = fixtures::tempdir();
    let image = dir.path().join("app");
    std::fs::write(&image, b"old image").unwrap();
    let mut tracker = SourceTracker::new(fingerprint(&image).unwrap(), image.clone());
    std::fs::write(&image, b"failed image").unwrap();
    tracker.reject(fingerprint(&image).unwrap());
    assert!(tracker.inspect("root", &image).unwrap().is_none());
    tracker.retry(&image);
    assert!(tracker.inspect("root", &image).unwrap().is_none());
    for _ in 0..100 {
        assert!(tracker.inspect("root", &image).unwrap().is_none());
    }
    assert_eq!(tracker.fingerprints_read(), 2);
    std::fs::write(&image, b"fixed image").unwrap();
    assert!(tracker.inspect("root", &image).unwrap().is_some());
}

#[tokio::test]
async fn bootstrap_guard_prevents_an_update_from_overtaking_activation() {
    use herdr_idle_inhibitor::{
        herdr::discovery::Registration,
        runtime::{
            paths::Paths,
            update::{activation_guard, pending},
        },
    };
    let dir = fixtures::tempdir();
    let paths = Paths {
        config: dir.path().join("config.toml"),
        state: dir.path().join("state"),
        runtime: dir.path().join("runtime"),
        endpoint: dir
            .path()
            .join("control.sock")
            .to_string_lossy()
            .into_owned(),
    };
    let registration = Registration {
        endpoint: dir.path().join("herdr.sock").to_string_lossy().into_owned(),
        herdr_bin: std::env::current_exe().unwrap(),
        plugin_root: dir.path().into(),
        env: Default::default(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        bus: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
    };
    let guard = activation_guard(&paths, &registration)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !pending(&paths).unwrap(),
        "ordinary activation is not publication"
    );
    let updater = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(paths.runtime.join("update.lock"))
        .unwrap();
    assert!(matches!(
        updater.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    let other = activation_guard(&paths, &registration)
        .await
        .unwrap()
        .unwrap();
    drop(guard);
    assert!(matches!(
        updater.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(other);
    updater.try_lock().unwrap();
    assert!(pending(&paths).unwrap());
    assert!(
        activation_guard(&paths, &registration)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        std::fs::read_dir(paths.runtime.join("pending"))
            .unwrap()
            .count(),
        1
    );
}
