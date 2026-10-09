"""Pure packaging contract tests; no native power calls or release publication."""
import copy
from pathlib import Path
import tomllib
import unittest
from package import TARGETS, bundle_manifest, vendor_configuration


class ManifestTests(unittest.TestCase):
    def test_source_and_native_build_free_manifests(self):
        root = Path(__file__).resolve().parents[1]
        source = tomllib.loads((root / "herdr-plugin.toml").read_text())
        cargo = tomllib.loads((root / "Cargo.toml").read_text())
        self.assertEqual(source["version"], cargo["package"]["version"])
        self.assertEqual(source["build"][0]["command"], ["cargo", "build", "--release", "--locked", "--target-dir", "target"])
        self.assertEqual(source["build"][1]["command"], ["./target/release/herdr-idle-inhibitor", "_prepare_update"])
        self.assertEqual(len(source["build"]), 2)
        self.assertEqual(source["min_herdr_version"], "0.9.3")
        self.assertEqual({r["on"] for r in source["events"]}, {"pane.agent_status_changed", "pane.agent_detected", "pane.exited", "pane.closed", "workspace.created", "workspace.closed"})
        for target, (platform, filename) in TARGETS.items():
            with self.subTest(target=target):
                expected = copy.deepcopy(source)
                del expected["build"]
                expected["platforms"] = [platform]
                for section in ["startup", "events", "actions", "panes"]:
                    for row in expected[section]:
                        row["command"][0] = f"./target/release/{filename}"
                generated = tomllib.loads(bundle_manifest(source, target))
                self.assertEqual(generated, expected)
                self.assertEqual(generated["actions"][0]["id"], "show")
                self.assertEqual(generated["panes"][0]["id"], "status")
                self.assertEqual(generated["panes"][0]["placement"], "popup")
                for section in ["actions", "panes"]:
                    ids = [r["id"] for r in generated[section]]
                    self.assertEqual(len(ids), len(set(ids)))
        self.assertIn("build", source)  # Rendering never mutates the source manifest.

    def test_vendor_config_replaces_escaped_windows_and_unix_temporary_paths(self):
        for path in ["/tmp/build/vendor", r"C:\Users\runner\AppData\Local\Temp\build\vendor"]:
            import json
            stdout = '[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = ' + json.dumps(path) + '\n'
            config = tomllib.loads(vendor_configuration(stdout))
            self.assertEqual(config["source"]["vendored-sources"]["directory"], "vendor")
            self.assertEqual(config["source"]["crates-io"]["replace-with"], "vendored-sources")

    def test_unexpected_entrypoint_is_not_silently_rewritten(self):
        source = {"platforms": ["linux"], "startup": [{"command": ["bash", "wrapper"]}]}
        with self.assertRaises(ValueError):
            bundle_manifest(source, "x86_64-unknown-linux-gnu")

    def test_windows_zip_and_unix_tar_share_verifier_archive_and_checksum_selection(self):
        import tarfile
        import tempfile
        import zipfile
        from package import archive_path, checksum_path
        from verify_bundle import extract_bundle
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "marker"
            marker.write_text("native bundle")
            for target in TARGETS:
                with self.subTest(target=target):
                    archive = archive_path(root, "bundle", target)
                    self.assertEqual(archive.name, "bundle.zip" if target.endswith("windows-msvc") else "bundle.tar.gz")
                    self.assertEqual(checksum_path(archive).name, archive.name + ".sha256")
                    if archive.suffix == ".zip":
                        with zipfile.ZipFile(archive, "w") as handle:
                            handle.write(marker, "bundle/marker")
                    else:
                        with tarfile.open(archive, "w:gz") as handle:
                            handle.add(marker, arcname="bundle/marker")
                    output = root / target
                    extract_bundle(archive, output)
                    self.assertEqual((output / "bundle/marker").read_text(), "native bundle")


if __name__ == "__main__":
    unittest.main()
