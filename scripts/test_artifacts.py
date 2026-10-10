"""Best-effort failure evidence for the standalone Linux legacy process test."""
import hashlib
import json
import os
from pathlib import Path
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SUFFIXES = {".json", ".toml", ".log", ".stdout", ".stderr", ".plugins"}


def preserve(directory, children, context, binaries, reason, artifacts=None):
    """Call before fixture cleanup; never query or kill a reported monitor PID."""
    try:
        processes = []
        for child in children:
            row = {"pid": child.pid, "command": child.args, "exit_code": child.poll()}
            row["state"] = "running" if row["exit_code"] is None else "exited"
            if row["state"] == "running":
                proc = Path("/proc") / str(child.pid)
                row["os"] = {}
                for name in ("stat", "status", "io", "wchan"):
                    try:
                        row["os"][name] = (proc / name).read_text()
                    except OSError as error:
                        row["os"][name + "_error"] = str(error)
                row["os"]["fds"] = []
                try:
                    for descriptor in list((proc / "fd").iterdir())[:64]:
                        try:
                            row["os"]["fds"].append({"fd": descriptor.name, "target": os.readlink(descriptor), "info": (proc / "fdinfo" / descriptor.name).read_text()})
                        except OSError:
                            pass  # A child can close descriptors or exit while observed.
                except OSError as error:
                    row["os"]["fds_error"] = str(error)
            processes.append(row)
        base = Path(artifacts or os.environ.get("HERDR_TEST_ARTIFACTS", ROOT / "dist/test-failures"))
        base.mkdir(parents=True, exist_ok=True)
        destination = Path(tempfile.mkdtemp(prefix="legacy-update-", dir=base))
        omissions, copied, total = [], 0, 0
        for parent, directories, files in os.walk(directory, followlinks=False):
            directories[:] = [name for name in directories if name not in {"bin", "target"} and not (Path(parent) / name).is_symlink()]
            for name in files:
                source = Path(parent) / name
                if source.is_symlink() or not source.is_file() or source.suffix not in SUFFIXES:
                    continue
                if copied >= 128 or total >= 8 * 1024 * 1024:
                    omissions.append("capture file/byte limit reached")
                    break
                try:
                    target = destination / "files" / source.relative_to(directory)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    limit = min(512 * 1024, 8 * 1024 * 1024 - total)
                    with source.open("rb") as stream:
                        size = source.stat().st_size
                        if size > limit:
                            stream.seek(size - limit)
                            omissions.append(f"truncated tail: {source} ({size} bytes)")
                        data = stream.read(limit)
                    target.write_bytes(data)
                    copied += 1
                    total += len(data)
                except OSError as error:
                    omissions.append(f"copy unavailable: {source}: {error}")
        identities = []
        for binary in binaries:
            row = {"path": str(binary)}
            try:
                row["bytes"] = binary.stat().st_size
                with binary.open("rb") as stream:
                    row["sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
            except OSError as error:
                row["hash_error"] = str(error)
            identities.append(row)
        report = {"schema_version": 1, "test": "legacy-update", "reason": reason, "fixture": str(directory), "context": context, "processes": processes, "binaries": identities, "copied_files": copied, "copied_bytes": total, "omissions": omissions, "ci_commit": os.environ.get("GITHUB_SHA")}
        (destination / "report.json").write_text(json.dumps(report, indent=2))
        print(f"test failure diagnostics: {destination / 'report.json'}", flush=True)
        return destination
    except Exception as error:
        print(f"could not preserve test failure diagnostics: {error}; original failure is unchanged", flush=True)
        return None
