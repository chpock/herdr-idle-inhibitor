# P5–P7 Integrated Final Review

## Review

- **Correct:** The current implementation preserves idle-only scope, separates observations from native ownership, and keeps public status reads operationally read-only.
- **Fixed:** All six reported defects and the minor status-document wording issue are closed by fresh source inspection below. Fixes were applied by the parent, not this reviewer.
- **Current findings:** No issues found.
- **Merge verdict: OK with notes — software-source review only.** Mandatory verification gaps remain; this is neither complete release acceptance nor authorization to commit or publish.

### Review boundary

Reviewed the uncommitted tree against the original implementation contract, amended validation requirements and public API contract. The integrated inspection covered implementation, tests, schema, manifest, packaging, CI, documentation and fixture/evidence provenance. After recovery, all correction paths and final logs were freshly reread. Earlier review reports and the parent’s findings ledger were not treated as proof of correctness.

No source edits, shell commands, installation, power changes, commits or delegation were performed.

## Verified finding closures

| Finding | Requirement and original root cause | Fresh closure evidence |
|---|---|---|
| **F1 — P1: Windows bundle verification failed before execution** | Plan  `docs/implementation-plan.md:408`; P7, `docs/implementation-validation.md:176-177`. Packaging produced ZIP, but verification unconditionally selected/read tar. | Shared archive/checksum selection: `scripts/package.py:22-28,124-134`; ZIP/tar extraction and verification: `scripts/verify_bundle.py:16-36`. Actual archive regression covers all targets: `scripts/test_package.py:49-72`. |
| **F2 — P1: Unsaved Pause lacked a persistent prominent warning** | Plan 7.2, `docs/implementation-plan.md:307`, and 8.1:341. Rendering ignored shared persistence state; another notice or reopened popup could conceal the failure. | Warning derives from shared control state, including resize-only rendering: `src/ui.rs:63-74,300-312`. It cannot be replaced by an “Applied” notice. Regression checks Main/Settings across four geometries and empty/success notices: `tests/ui.rs:60-113`. |
| **F3 — P2: Required event hints were missing** | Plan 4.2, `docs/implementation-plan.md:113`. The manifest omitted detection and pane-close hooks, although periodic polling remained operational. | Exact events `pane.agent_detected` and `pane.closed` now invoke `_ensure`: `herdr-plugin.toml:18-24`. Complete hook set and generated native manifests are checked at `scripts/test_package.py:17-27`. Hooks remain hints, not work authority. |
| **F4 — P2: README overstated Herdr compatibility** | Plan 4.1, `docs/implementation-plan.md:106`. README promised “0.9.3+” while the parser accepted only 0.9.3. | `README.md:3` now states the exact qualification boundary without weakening `src/herdr/protocol.rs:44-49`. Regression accepts 0.9.3 and rejects older/newer unqualified versions: `tests/protocol.rs:53-73`. |
| **F5 — P2: Failed configuration operations disappeared from shared diagnostics** | Plan 7.2, `docs/implementation-plan.md:305-308`. Reload/apply returned failures without retaining them for the existing status projection. | `src/runtime/config.rs:116-130,167-179` retains failures without replacing last-valid settings; successful operations clear them. Existing public projection remains generic and non-sensitive: `src/controller.rs:338-342`. Store regression: `tests/config.rs:48-82`; repeated shared-status/recovery regression: `src/controller.rs:1075-1158`. |
| **F6 — P1: Small windows accepted invisible actions; Escape inconsistently closed** | Plan 8.1, `docs/implementation-plan.md:325,339`. Wrapped content could hide focused controls while input remained active; secondary-view Escape navigated instead of closing. | Focus-aware, nonwrapping interactive viewport: `src/ui.rs:278-299`. Minimum geometry and dismissal-only gate: `src/ui.rs:30-40,370-376`. Escape/q/Ctrl+C close before view-specific handling. Regressions: `tests/ui.rs:60-149`. Installed-Herdr interaction remains an explicit manual gap. |

**Minor documentation correction:** `docs/status-api.md:3` now correctly identifies the implemented v1 contract/schema without implying native or physical certification.

## Integrated correctness and evidence

- **Status purity/privacy:** `src/main.rs:38-64` performs only the existing-monitor query. `src/controller.rs:465-495` excludes `GetStatus` from gap handling and policy evaluation. Public projection excludes private endpoint/configuration details (`src/controller.rs:363-405,503-510`). Popup refresh is read-only; activation occurs through explicit Retry (`src/ui.rs:347-358,424-440`).
- **Scope and ownership:** Native calls remain logind `idle/block`, GNOME flag `4`, KDE flag `1`, macOS `PreventUserIdleSystemSleep`, and Windows `PowerRequestSystemRequired` (`src/backend/linux.rs:86-107`, `src/backend/macos.rs:21`, `src/backend/windows.rs:34,54`). Popup exit restores terminal state without owning or releasing the monitor’s request (`src/ui.rs:314-330,374-376`).
- **CI targeting:** Four native runner/target pairs, immutable action references, host-target checking and explicit SDK selection are present in `.github/workflows/ci.yml:18-61`. Native probes and bundle verification are configured at lines 78-90. Configuration is not execution evidence.
- **Replay traceability:** The minimized capture contains six normalized agent rows, exactly two working (`tests/fixtures/herdr-0.9.3/agent-list.json:5-29`), with capture metadata in `provenance.json:2-8`. Replay uses the controller and local transport; completion is explicitly synthetic (`tests/integration.rs:327-451`).
- **Measured query isolation:** Final raw counters show `status_reads` **11 → 112**, while snapshot, discovery and native acquisition/release counters remain unchanged (`docs/validation/logs/replay-counters.txt:8`). Captured replay reports one fake acquisition and one release following synthetic completion (line 7). This proves the recorded software experiment, not physical sleep prevention.

### Validation attribution

Freshly inspected **parent-executed** logs establish:

- **47 Linux Rust tests passed**, including 14 library tests and the new regressions: `docs/validation/logs/rust-tests.txt:5-120`.
- **4 packaging tests passed:** `docs/validation/logs/packaging-tests.txt:1-5`.
- Clippy, release build, Windows GNU/macOS arm64 cross-checks completed successfully in their respective logs.
- Linux bundle verification completed its checksum, build-free executable and cold offline-source checks: `docs/validation/logs/package-verification.txt:219-221`.

Formatting, Python compilation and actionlint logs are empty; their successful exit status is parent-attested, not independently established by empty output.

Reviewer-run AFT inspection reported zero scoped diagnostic errors/warnings, but metrics/TODO collection encountered a cache lease timeout. It is not a substitute for compiler/test execution.

## Remaining verification and release gates

These are **unperformed checks, not additional reproduced source defects**:

1. Run and retain all four hosted native jobs, including Windows MSVC linking/security/API tests and both macOS architectures. Linux-hosted cross-checks do not satisfy this gate.
2. Complete installed-Herdr acceptance: activation with existing work, popup controls/close/reopen/retry, shared persistent Pause, update/uninstall and clean-machine bundle/security-warning behavior.
3. Complete the broader real-workflow capture/replay matrix required by `docs/implementation-validation.md:125-140`. One captured snapshot plus synthetic completion does not establish all requested real transitions.
4. Obtain separately authorized Linux profile power trials: idle-sleep counterfactual, display-off/lock, manual sleep, lid, resume and workload progress. Real logind ownership cleanup is not sleep-effectiveness certification.
5. Physical Windows/macOS absence remains owner-accepted for this delivery under the native-runner amendment, not a physical pass. The Windows battery limitation and precise KDE suppression → owner death → reallow exception remain narrowly bounded; neither waives unrelated checks.

**Software review is complete. Full implementation/release acceptance remains incomplete pending applicable gates. No owner commit, push or publication approval is inferred.**