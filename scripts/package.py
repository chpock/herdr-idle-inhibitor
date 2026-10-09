#!/usr/bin/env python3
"""Development-only native bundle builder; no installation or publication."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

TARGETS = {
    "x86_64-unknown-linux-gnu": ("linux", "herdr-idle-inhibitor"),
    "aarch64-apple-darwin": ("macos", "herdr-idle-inhibitor"),
    "x86_64-apple-darwin": ("macos", "herdr-idle-inhibitor"),
    "x86_64-pc-windows-msvc": ("windows", "herdr-idle-inhibitor.exe"),
}


def archive_path(output, stem, target):
    suffix = ".zip" if target.endswith("windows-msvc") else ".tar.gz"
    return output / f"{stem}{suffix}"


def checksum_path(archive):
    return archive.with_suffix(archive.suffix + ".sha256")


def bundle_manifest(source, target):
    """Keep every manifest entry/ID; only remove builds and fix native extension."""
    source = dict(source)
    source.pop("build", None)
    source["platforms"] = [TARGETS[target][0]]
    filename = TARGETS[target][1]
    lines = []
    for key, value in source.items():
        if isinstance(value, list) and value and isinstance(value[0], dict):
            for row in value:
                lines.extend(["", f"[[{key}]]"])
                for name, field in row.items():
                    if name == "command":
                        if field[0] != "./target/release/herdr-idle-inhibitor":
                            raise ValueError("unexpected source entrypoint")
                        field = [f"./target/release/{filename}", *field[1:]]
                    lines.append(f"{name} = {json.dumps(field, ensure_ascii=False)}")
        else:
            lines.append(f"{key} = {json.dumps(value, ensure_ascii=False)}")
    return "\n".join(lines) + "\n"


def vendor_configuration(stdout):
    """Parse Cargo's escaped paths before replacing the temporary vendor location."""
    config = tomllib.loads(stdout)
    config["source"]["vendored-sources"]["directory"] = "vendor"
    lines = []
    for name, fields in config["source"].items():
        lines.append(f"[source.{json.dumps(name)}]")
        lines.extend(f"{key} = {json.dumps(value)}" for key, value in fields.items())
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    source = tomllib.loads((root / "herdr-plugin.toml").read_text())
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    if source["version"] != cargo["package"]["version"]:
        parser.error("Cargo and plugin versions differ")
    binary = args.binary.resolve(strict=True)
    # Native CI invokes the native output, rather than labeling cross binaries as run.
    version = subprocess.check_output([str(binary), "--version"], text=True).strip()
    if version != f"herdr-idle-inhibitor {source['version']}":
        parser.error("binary version does not match source manifest")
    filename = TARGETS[args.target][1]
    # Verify the target architecture from the executable header, independently of its name.
    data = binary.read_bytes()
    if args.target.endswith("linux-gnu"):
        matches = data[:4] == b"\x7fELF" and data[4:6] == b"\x02\x01" and int.from_bytes(data[18:20], "little") == 62
    elif args.target.endswith("apple-darwin"):
        cpu = 0x100000C if args.target.startswith("aarch64") else 0x1000007
        matches = data[:4] == b"\xcf\xfa\xed\xfe" and int.from_bytes(data[4:8], "little") == cpu
    else:
        offset = int.from_bytes(data[60:64], "little")
        matches = data[:2] == b"MZ" and data[offset:offset + 4] == b"PE\0\0" and int.from_bytes(data[offset + 4:offset + 6], "little") == 0x8664
    if not matches:
        parser.error("binary format/architecture does not match target")
    args.output.mkdir(parents=True, exist_ok=True)
    stem = f"herdr-idle-inhibitor-{source['version']}-{args.target}"
    with tempfile.TemporaryDirectory(prefix="herdr-package-") as tmp:
        tmp = Path(tmp)
        package = tmp / stem
        executable = package / "target" / "release" / filename
        executable.parent.mkdir(parents=True)
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        (package / "herdr-plugin.toml").write_text(bundle_manifest(source, args.target))
        import os
        environment = {key: os.environ[key] for key in ["ImageOS", "ImageVersion", "GITHUB_SHA", "RUNNER_OS", "RUNNER_ARCH"] if key in os.environ}
        (package / "build-environment.json").write_text(json.dumps({"target": args.target, "version": source["version"], "environment": environment}, indent=2) + "\n")
        for name in ["README.md", "LICENSE"]:
            shutil.copy2(root / name, package / name)
        for name in ["docs", "schemas"]:
            shutil.copytree(root / name, package / name)
        matching = tmp / "matching-source"
        matching.mkdir()
        for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "herdr-plugin.toml", "README.md", "LICENSE"]:
            shutil.copy2(root / name, matching / name)
        for name in ["src", "tests", "examples", "scripts", "docs", "schemas"]:
            shutil.copytree(root / name, matching / name, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
        # Preserve third-party source, checksums and license notices for GPL distribution.
        vendor = subprocess.run(["cargo", "vendor", "--locked", "--versioned-dirs", str(matching / "vendor")], cwd=root, check=True, text=True, stdout=subprocess.PIPE)
        config = vendor_configuration(vendor.stdout)
        (matching / ".cargo").mkdir()
        (matching / ".cargo" / "config.toml").write_text(config)
        with tarfile.open(package / "matching-source.tar.gz", "w:gz") as archive:
            archive.add(matching, arcname="herdr-idle-inhibitor-source")
        output = archive_path(args.output, stem, args.target)
        if args.target.endswith("windows-msvc"):
            with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(package.rglob("*")):
                    if path.is_file():
                        archive.write(path, path.relative_to(tmp))
        else:
            with tarfile.open(output, "w:gz") as archive:
                archive.add(package, arcname=stem)
        digest = hashlib.sha256(output.read_bytes()).hexdigest()
        checksum_path(output).write_text(f"{digest}  {output.name}\n")
        print(output)


if __name__ == "__main__":
    main()
