#!/usr/bin/env python3
"""Linux process regression using the immutable pre-update baseline executable.

Run in a private /tmp namespace locally, or on an otherwise empty CI runner.
Neither scenario acquires a native sleep request or changes host power policy.
"""
import argparse
import asyncio
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid
from test_artifacts import preserve

ROOT = Path(__file__).resolve().parents[1]
BASELINE = "943e2fd18ac29ae19f4bb57839b4a87a96472690"


def private_json(path, value):
    path.write_text(json.dumps(value))
    path.chmod(0o600)


async def query(endpoint, operation):
    reader, writer = await asyncio.wait_for(asyncio.open_unix_connection(endpoint), 2)
    try:
        request_id = str(uuid.uuid4())
        writer.write(json.dumps({"protocol_version": 1, "request_id": request_id, "operation": operation}).encode() + b"\n")
        await writer.drain()
        reply = json.loads(await asyncio.wait_for(reader.readline(), 2))
        assert reply["request_id"] == request_id and reply["protocol_version"] == 1
        return reply
    finally:
        writer.close()
        await writer.wait_closed()


async def wait_status(endpoint, predicate, timeout=15, diagnostics=None):
    deadline = time.monotonic() + timeout
    if diagnostics is not None:
        diagnostics["wait"] = {"kind": "status predicate", "endpoint": endpoint, "deadline_seconds": timeout}
    while True:
        try:
            reply = await query(endpoint, {"type": "GetStatus", "details": True})
            if diagnostics is not None:
                diagnostics["status.last_reply"] = reply
            if predicate(reply):
                return reply
        except (OSError, asyncio.TimeoutError) as error:
            if diagnostics is not None:
                diagnostics["status.last_error"] = {"type": type(error).__name__, "message": str(error), "errno": getattr(error, "errno", None)}
        assert time.monotonic() < deadline, f"legacy recovery status deadline expired; last query evidence: {diagnostics}"
        await asyncio.sleep(0.025)


class InjectedFailure(Exception):
    pass


async def scenario(legacy, current, directory, publication_succeeds, inject_failure=False):
    runtime = Path("/tmp").resolve() / f"herdr-idle-inhibitor-{os.geteuid()}"
    endpoint = str(runtime / "control.sock")
    try:
        await query(endpoint, {"type": "GetStatus"})
    except (FileNotFoundError, ConnectionRefusedError):
        pass  # Normal singleton startup, not this fixture, removes stale paths.
    else:
        raise AssertionError("refusing to touch an existing monitor")
    config = directory / "config.toml"
    config.write_text("schema_version = 1\npaused = false\nrelease_delay_secs = 0\nadditional_endpoints = []\n[linux]\nbackend = 'auto'\nhypridle_integration_confirmed = false\n")
    state = directory / "state"
    state.mkdir(mode=0o700)
    fake = directory / "herdr"
    subprocess.run(["rustc", str(ROOT / "tests/support/fake_cli.rs"), "-o", str(fake)], check=True)
    candidate_digest = hashlib.sha256(current.read_bytes()).hexdigest()
    cached = state / "bin" / candidate_digest / "herdr-idle-inhibitor"
    (state / "bin").mkdir(mode=0o700)
    cached.parent.mkdir(mode=0o700)
    shutil.copyfile(current, cached)
    cached.chmod(0o700)
    registrations, servers, children, enabled, counters = [], [], [], [True, True], [0, 0]
    commits = ["old-commit", "old-commit"]
    capture = json.loads((ROOT / "tests/fixtures/herdr-0.9.3/agent-list.json").read_text())["result"]
    expected = 2 * sum(agent["agent_status"] == "working" for agent in capture["agents"])
    environment = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_")}
    environment.update(HERDR_IDLE_INHIBITOR_CONFIG=str(config), HERDR_IDLE_INHIBITOR_STATE=str(state), XDG_CURRENT_DESKTOP="unsupported-test-profile", TOKIO_WORKER_THREADS="2")

    def plugin(index):
        return {"plugin_id": "herdr-idle-inhibitor", "plugin_root": str(registrations[index]["plugin_root"]), "enabled": enabled[index], "source": {"resolved_commit": commits[index]}}

    def registry(index):
        private_json(Path(registrations[index]["env"]["HERDR_CONFIG_PATH"] + ".plugins"), {"result": {"plugins": [plugin(index)]}})

    async def serve(index, reader, writer):
        try:
            request = json.loads(await reader.readline())
            method = request["method"]
            if method == "plugin.list":
                result = {"type": "plugin_list", "plugins": [plugin(index)]}
            elif method in ("plugin.disable", "plugin.enable"):
                enabled[index] = method == "plugin.enable"
                registry(index)
                result = {"type": "plugin_enabled" if enabled[index] else "plugin_disabled"}
            elif method == "ping":
                result = {"type": "pong", "version": "0.9.3"}
            elif method == "agent.list":
                counters[index] += 1
                result = capture
            else:
                raise AssertionError(method)
            writer.write(json.dumps({"id": request["id"], "result": result}).encode() + b"\n")
            await writer.drain()
        finally:
            writer.close()
            await writer.wait_closed()

    log = directory / "legacy.log"
    diagnostics = {"baseline_commit": BASELINE, "publication_succeeds": publication_succeeds, "candidate_digest": candidate_digest, "phase": "starting legacy authority"}
    try:
        for index in range(2):
            root = directory / f"root-{index}"
            installed = root / "target/release/herdr-idle-inhibitor"
            installed.parent.mkdir(parents=True)
            shutil.copyfile(legacy, installed)
            installed.chmod(0o700)
            address = str(directory / f"herdr-{index}.sock")
            sessions = directory / f"sessions-{index}.json"
            private_json(sessions, {"sessions": [{"name": "default", "running": True, "socket_path": address}]})
            registrations.append({"endpoint": address, "herdr_bin": str(fake), "plugin_root": str(root), "env": {"HERDR_CONFIG_PATH": str(sessions)}, "desktop": "unsupported-test-profile", "bus": environment.get("DBUS_SESSION_BUS_ADDRESS")})
            registry(index)
            servers.append(await asyncio.start_unix_server(lambda r, w, i=index: serve(i, r, w), address))
        with log.open("wb") as output:
            children.append(subprocess.Popen([str(Path(registrations[0]["plugin_root"]) / "target/release/herdr-idle-inhibitor"), "_serve"], env=environment, cwd=directory, stdout=output, stderr=output))
        initial = await wait_status(endpoint, lambda r: r["details"] is not None, diagnostics=diagnostics)
        assert initial["details"]["config_path"] == str(config), "startup did not become the fixture authority"
        for registration in registrations:
            reply = await query(endpoint, {"type": "RegisterAndRefresh", "registration": registration})
            assert reply["error"] is None
        before = await wait_status(endpoint, lambda r: r["status"]["observation"]["working_agents_observed"] == expected, diagnostics=diagnostics)
        assert before["details"].get("runtime") is None, "must really exercise baseline legacy IPC"
        # A valid external edit makes the baseline's expected hash stale. Pause
        # applies in memory but its attempted save cannot overwrite that edit.
        config.write_text(config.read_text() + "# deliberate external edit\n")
        paused = await query(endpoint, {"type": "SetPaused", "paused": True})
        assert paused["status"]["control"]["paused"] is True
        assert paused["status"]["control"]["pause_persisted"] is False
        original = config.read_bytes()
        counts_before = list(counters)
        if inject_failure:
            raise InjectedFailure("deliberate failure before legacy migration")
        diagnostics["phase"] = "awaiting updater readiness/publication"
        ticket = directory / "update"
        ticket.mkdir(mode=0o700)
        paths = {"config": str(config), "state": str(state), "runtime": str(runtime), "endpoint": endpoint}
        plan = {"paths": paths, "candidate": {"path": str(cached), "digest": candidate_digest}, "publication": {"executable": str(Path(registrations[0]["plugin_root"]) / "target/release/herdr-idle-inhibitor"), "digest": candidate_digest, "plugin_root": registrations[0]["plugin_root"], "expected_commit": "new-commit"}, "base": registrations[0], "handoff": None, "prepare": True, "direct": False, "discover_root": False, "source_root": None, "timeout_ms": 500 if not publication_succeeds else 5000}
        private_json(ticket / "plan.json", plan)
        with log.open("ab") as output:
            children.append(subprocess.Popen([str(cached), "_upgrade_worker", str(ticket / "plan.json")], env=environment, cwd=state, stdout=output, stderr=output))
        deadline = time.monotonic() + 45
        while not (ticket / "ready.json").exists():
            assert not (ticket / "error.json").exists(), (ticket / "error.json").read_text() if (ticket / "error.json").exists() else ""
            diagnostics["wait"] = {"kind": "updater ready marker", "marker": str(ticket / "ready.json"), "deadline_seconds": 45}
            assert time.monotonic() < deadline, log.read_text()
            await asyncio.sleep(0.025)
        if publication_succeeds:
            installed = Path(plan["publication"]["executable"])
            shutil.copyfile(cached, installed)
            enabled[0] = True
            # Publish a new artifact/revision only in A; B remains the old checkout.
            commits[0] = "new-commit"
            registry(0)
        marker = ticket / ("complete.json" if publication_succeeds else "error.json")
        diagnostics["wait"] = {"kind": "updater final marker", "marker": str(marker), "deadline_seconds": 45}
        while not marker.exists():
            assert time.monotonic() < deadline, log.read_text()
            await asyncio.sleep(0.025)
        diagnostics["phase"] = "verifying recovered monitor and captured observations"
        after = await wait_status(endpoint, lambda r: r["status"]["observation"]["working_agents_observed"] == expected and r["status"]["monitor"]["instance_id"] != before["status"]["monitor"]["instance_id"], diagnostics=diagnostics)
        assert after["status"]["control"]["paused"] is True, "unsaved Pause lost during legacy transition"
        assert after["status"]["control"]["pause_persisted"] is False
        assert after["status"]["diagnostics"]["counters"]["native_acquires"] == 0
        assert after["details"]["runtime"]["legacy_endpoints"] == [registrations[1]["endpoint"]]
        assert all(new > old for new, old in zip(counters, counts_before)), "both roots need fresh captured observations"
        assert enabled == [True, True]
        assert config.read_bytes() == original, "transition must not overwrite configuration"
        diagnostics["phase"] = "verifying stable recovered instance"
        instance = after["status"]["monitor"]["instance_id"]
        await asyncio.sleep(6)
        assert (await query(endpoint, {"type": "GetStatus"}))["status"]["monitor"]["instance_id"] == instance
        diagnostics["phase"] = "fixture cooperative shutdown"
        await query(endpoint, {"type": "PrepareUpgrade"})
        deadline = time.monotonic() + 10
        while Path(endpoint).exists():
            assert time.monotonic() < deadline
            await asyncio.sleep(0.025)
        print(json.dumps({"baseline_commit": BASELINE, "publication_succeeded": publication_succeeds, "known_roots": 2, "working_agents_observed": expected, "captured_snapshot_calls_before": counts_before, "captured_snapshot_calls_after": counters, "unsaved_pause_preserved": True, "configuration_unchanged": True, "native_acquires": 0, "stable_recovered_instance": True}))
    except BaseException as error:
        if not isinstance(error, InjectedFailure):
            diagnostics["captured_snapshot_calls"] = list(counters)
            preserve(directory, children, diagnostics, [legacy, current], f"{type(error).__name__}: {error}")
        print(log.read_text() if log.exists() else "legacy fixture did not start", flush=True)
        raise
    finally:
        try:
            reply = await query(endpoint, {"type": "GetStatus", "details": True})
            if (reply.get("details") or {}).get("config_path") == str(config):
                # Only stop the fixture authority authenticated by its private
                # configuration path, never a PID returned by arbitrary status.
                if reply["details"].get("runtime") is not None:
                    await query(endpoint, {"type": "PrepareUpgrade"})
                for index in range(len(registrations)):
                    enabled[index] = False
                    registry(index)
        except (FileNotFoundError, ConnectionRefusedError):
            pass  # Successful cooperative shutdown already removed the owner.
        except Exception as cleanup_error:
            print(f"fixture cooperative cleanup failed: {cleanup_error}", flush=True)
        finally:
            for child in children:
                if child.poll() is None:
                    child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            for server in servers:
                server.close()
                await server.wait_closed()
        assert all(child.poll() is not None for child in children)
        if inject_failure:
            try:
                await query(endpoint, {"type": "GetStatus"})
            except (FileNotFoundError, ConnectionRefusedError):
                pass
            else:
                raise AssertionError("injected failure left the legacy authority running")


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    args = parser.parse_args()
    assert os.name == "posix" and os.uname().sysname == "Linux"
    with tempfile.TemporaryDirectory(prefix="legacy-cleanup-", dir="/tmp") as directory:
        try:
            await scenario(args.legacy.resolve(strict=True), args.current.resolve(strict=True), Path(directory).resolve(), False, inject_failure=True)
        except InjectedFailure:
            print(json.dumps({"injected_pre_migration_failure": True, "original_error_preserved": True, "legacy_child_terminated": True, "fixture_servers_closed": True}))
        else:
            raise AssertionError("expected the injected legacy fixture failure")
    for success in [False, True]:
        with tempfile.TemporaryDirectory(prefix="legacy-update-", dir="/tmp") as directory:
            await scenario(args.legacy.resolve(strict=True), args.current.resolve(strict=True), Path(directory).resolve(), success)


if __name__ == "__main__":
    asyncio.run(main())
