#!/usr/bin/env python3
"""Verify an unpublished native bundle and offline matching source on its native runner."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile
from package import TARGETS, archive_path, checksum_path


def extract_bundle(archive, destination):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            handle.extractall(destination)
    else:
        with tarfile.open(archive) as handle:
            handle.extractall(destination, filter="data")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    name = f"herdr-idle-inhibitor-{version}-{args.target}"
    archive = archive_path(args.output, name, args.target).resolve()
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == checksum_path(archive).read_text().split()[0]
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        extract_bundle(archive, root)
        bundle = root / name
        manifest = tomllib.loads((bundle / "herdr-plugin.toml").read_text())
        platform, binary = TARGETS[args.target]
        assert "build" not in manifest and manifest["platforms"] == [platform]
        home = root / "query-home"
        home.mkdir()
        environment = {key: value for key, value in os.environ.items() if not key.startswith("HERDR_")}
        environment.update(HOME=str(home), XDG_CONFIG_HOME=str(home / "config"), XDG_STATE_HOME=str(home / "state"), PATH=str(root / "no-tools"))
        result = subprocess.run([str(bundle / "target/release" / binary), "status", "--json"], env=environment, capture_output=True, text=True, timeout=10)
        status = json.loads(result.stdout)
        assert result.returncode == 3 and status["available"] is False and status["observation"]["working_agents_observed"] is None, (result.returncode, result.stdout, result.stderr)
        assert not list(home.iterdir())
        with tarfile.open(bundle / "matching-source.tar.gz") as handle:
            handle.extractall(root, filter="data")
        source = root / "herdr-idle-inhibitor-source"
        config = tomllib.loads((source / ".cargo/config.toml").read_text())
        assert config["source"]["vendored-sources"]["directory"] == "vendor"
        environment = dict(os.environ, CARGO_HOME=str(root / "empty-cargo-home"), CARGO_TARGET_DIR=str(root / "offline-target"))
        # Cargo searches configuration from cwd, not --manifest-path's directory.
        subprocess.run(["cargo", "check", "--offline", "--locked", "--all-targets"], cwd=source, env=environment, check=True, timeout=900)
        print(json.dumps({"checksum_matches": True, "build_free_manifest": True, "status_without_cargo_or_herdr_context": True, "unavailable_not_zero": True, "created_query_home_entries": 0, "vendored_source_offline_check": True}))


if __name__ == "__main__":
    main()
