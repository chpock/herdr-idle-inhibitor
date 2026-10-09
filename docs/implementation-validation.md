# Implementation sequence, evidence, and acceptance gates

Status: authorized implementation contract. The verification amendment below supersedes the original physical-environment stop rules; evidence is recorded separately and is never inferred from this plan.

## Owner-approved verification amendment

The owner authorized full implementation without physical Windows/macOS access. Native builds and automated unit/integration tests on GitHub runners replace their physical blocking gates for this delivery. README must state that runtime testing is Linux-only and other platforms are expected to work, with issues requested for failures. Physical sleep/lock/lid/battery certification remains unperformed. CI configuration alone is not a passing CI run. See [implementation record](implementation-notes.md). The original matrix below remains the future manual acceptance procedure, not a reason to stop software work solely because Windows/macOS hardware is unavailable.

This is the verification half of [the implementation plan](implementation-plan.md). Requirements come from [the agreed scope](scope-decisions.md), not from a reviewer inventing new features. Each phase is test-first: write a failing contract/behavior test, implement the smallest correct layer, run its gate, and obtain fresh independent review. Do not commit without explicit permission.

## Gate rules

- A phase passes only on recorded observable results. A build, a mocked API return, a handle value, or the absence of log errors is not a native idle-sleep proof.
- Native probes exercise the selected real APIs; they are not temporary production backends or stronger fallback mechanisms.
- A required physical/desktop environment that is unavailable is **BLOCKED**, not a skipped test reported as passed. VM/cloud-host results cannot certify a physical lid, Modern Standby, or battery behavior they do not implement.
- P0 resolves the highest-risk native contracts before building the product around them. If P0 is blocked or fails, stop dependent implementation and report the missing environment/reproduced contradiction. Do not spend the rest of the project treating that uncertainty as an accepted limitation.
- The only already accepted behavior exception is possible idle sleep on Windows Modern Standby while on battery. Document a reproduction in that row without inventing a workaround. It does not waive other Windows lock/display/manual-sleep rows.
- Every failed mandatory contract needs either a root-cause fix at the responsible layer, a separately approved product change, or a blocker. No implicit platform removal, broad sleep lock, synthetic activity, or power-plan edit.
- Distinguish application behavior from Herdr's reported activity and from unrelated inhibitors. If Herdr itself does not report the agreed work, do not add agent-specific process/terminal heuristics to disguise that boundary.

## P0. Contract fixtures and native feasibility

**Purpose:** prove the API semantics that can invalidate the whole architecture, before implementing a polished daemon/UI.

Tests and evidence first:

1. Freeze Herdr `0.9.3` source identity and representative native transport/request/result fixtures, including Windows endpoint naming. Label synthetic schema fixtures as synthetic. Capture real data only with authorization and redact before persistence.
2. Establish real test environments for Hypridle 0.1.8, GNOME 51, PowerDevil 6.7.5, macOS, and Windows. Record OS/desktop/package versions, hardware architecture, display/lock/sleep configuration and, for Windows tests, the output of `powercfg /a` and the actual power source. This is test metadata, not a new runtime detector feature.
3. Write minimal native conformance tests around the intended production FFI/D-Bus calls: request, inspect ownership, release, process kill, explicit/lid sleep, display-off/lock, and wake/reacquisition. Test a separate workload process, not only whether the requesting process survives.
4. Include Windows display-off/Win+L/workload-progress cases as ordinary native conformance tests; direct API review resolved the special planning blocker, not these unrun tests. Check GNOME manual/lid paths early and confirm KDE's cookie-to-active delay and suppression. If Windows ignores a request, record applicable request-policy configuration before blaming session locking; do not change power policy to force a pass.
5. Verify short Herdr startup/event hooks, explicit first activation, and detached-child lifetime on native Windows as well as Unix. Use an isolated test plugin/configuration, not an unapproved change to the user's installed plugins.
6. Fix the reproducible build baseline: exact Rust toolchain, native targets, minimum tested OS/SDK/libc, and dependency versions compatible with those targets. Commit the resulting lock/toolchain files only at a separately approved commit boundary.

Artifacts to produce during implementation:

- `tests/native/` conformance harness and usage instructions; destructive/suspend tests are opt-in/ignored by default.
- `tests/fixtures/herdr/` minimal wire fixtures with provenance metadata.
- `docs/validation/native-matrix.md`, with one row per real environment/test and raw artifact locations.
- `docs/reviews/P0-native-feasibility.md`, with independently verified conclusions, gaps, and any blocker.

**Pass:** required native behaviors demonstrated, known Windows exception accurately distinguished, cleanup after forced process termination observed, and transport/lifecycle smoke tests pass. If a required experiment cannot run, P0 is not complete. Tests may be performed by an authorized operator on the corresponding machine; a source-code citation is not a replacement.

## P1. Pure model, timing, configuration, and status projection

Tests first:

- Table-test literal statuses, duplicate projections, two-server aggregation, empty versus failed observations, and complete versus partial coverage. Cold startup with an empty/unestablished scope is unknown, not none; adding an unverified root invalidates completeness. Test discovery/eligibility aging independently of fresh agent snapshots.
- Exercise every decision-table row with a fake suspend-aware clock. Prove grace deadlines do not slide on repeated idle/unknown/error messages. Cover a long suspend: expired resource retention ends, old in-flight replies are discarded, but the monitor gets one bounded revalidation round instead of exiting solely on pre-suspend discovery/eligibility ages. The round does not revive old inhibition.
- Verify that a positive observation from B keeps inhibition despite a failure/idle transition in A; a known nonworking observation clears A's old retainable work. If A finishes and B is unknown without ever having reported work, release after A's existing normal grace rather than retain forever. A cold unknown state cannot create a deadline or resource.
- Pause, Resume, cold start, invalid startup config, failed reload, invalid field/range/schema, external config edits, failed Pause persistence, and failed Resume persistence.
- Public JSON golden cases, nullability and exit mapping; unavailable is never zero work. Distinguish empty EOF from a partial/malformed frame and tracked endpoints from proven live processes. Generate a JSON Schema from the agreed projection and verify examples against it.
- Sequence/property tests for invariants: no acquire without fresh work, no acquire just for grace, no unknown-retention renewal, no duplicate native intent while state is unchanged.

Implement `model`, `policy`, `clock`, `config`, and the status projection without Herdr polling or actual power calls. No UI needs to exist yet.

**Pass:** deterministic tests without timing sleeps; all decision invariants and the public schema covered; independent review verifies the reducer rather than only its tests. Record `docs/reviews/P1-core.md`.

## P2. Runtime ownership, private IPC, and bootstrap

Tests first:

- Launch concurrent bootstraps from two or more independent Herdr contexts; exactly one owner/listener wins. Verify the persistent lock inode and safe stale-socket reclamation.
- Test long/unusual paths, spaces, Unicode, socket/pipe permissions, an unexpected file/symlink, a live-but-unresponsive owner, protocol mismatch, malformed/oversized frames and deadline expiry.
- A losing bootstrap cannot remove the winner's endpoint. A stale PID cannot cause a kill. A status query cannot create directories, start a child, read Herdr or acquire a native resource.
- Test parent exit, popup exit, early child failure, and stdout/stderr detachment. No inherited pipe keeps a Herdr hook waiting forever. Inspect the child environment and diagnostics to ensure full context/event payloads were not inherited or logged.
- On Windows test real named-pipe name mapping and creation-time ACLs, not just Linux Unix sockets with a Windows cfg build.
- Leak test: spawn a long-lived discovery child, terminate the owner, and prove that the child does not keep the singleton claim or native resource alive.

Implement local IPC/paths/singleton/bootstrap and controller wiring with fake Herdr/backend adapters.

**Pass:** multi-process tests on Unix and native Windows; exactly one owner and zero query-induced policy I/O; credential/permission failures stay failures. Record `docs/reviews/P2-runtime.md`.

## P3. Herdr observation and lifecycle integration

Tests first with a real local fake transport, then actual Herdr:

- Normal `agent.list`; API error; missing required data; ID/result mismatch; huge/truncated JSON; unexpected status; duplicate terminal projection with conflicting status; slow server and disconnect.
- Startup/event hints; coalesced burst; a hint arriving during a request; lost hints recovered by polling; at most one read per server and bounded cross-server concurrency.
- Old-generation result after reconnect/handoff, endpoint reuse, server disappears/reappears, separate named/default sessions, registered custom endpoint/root, discovery failure and legitimate empty discovery.
- Shared Pause across windows/servers; observation continues while paused. No selected workspace/focused-pane filtering.
- Enable into a running server requires the documented activation; registry disable/unlink exits; registry error gets bounded uncertainty, not a fake disable or eternal eligibility. Its deadline is based on last verified enabled evidence, does not renew on failures, and cannot authorize reacquisition after native loss.
- Last server gone versus all observations temporarily lost; monitor idle exit; wake/revalidation versus genuine disable; later bootstrap; crash with no subsequent hook has no false recovery guarantee. A deliberately frozen process retains OS-owned resources until resume/termination; do not claim the controller's timer runs while the process is stopped.
- Real Herdr client detach/reattach and inactive workspaces must still produce the scope the API actually promises. Record any upstream reporting defect separately.

Implement discovery/native request DTOs/per-server monitors and the manifest hooks. Keep native power behind the already verified adapter boundary.

**Pass:** deterministic integration suite plus recorded real Herdr trajectories; exact counters prove refresh origins and no duplicate ownership. Record `docs/reviews/P3-herdr.md`.

## P4. Owned native backends and recovery

Implement the P0-proven adapters, reusing rather than replacing their conformance tests. Each desktop/backend is reviewed independently before treating it as accepted.

Required tests:

- Balanced acquisition/release and zero repeated acquisitions on unchanged working snapshots; never pass forbidden flags/methods.
- Loss/restart of D-Bus service or connection; acquisition succeeded remotely but reply was lost; cookie release failure closes the owning connection; no clone keeps an orphan request alive.
- Pause/end-of-work arrives before a delayed successful acquisition; its result is released and not published as current.
- Error plus fresh work retries with bounded backoff; error/unknown alone does not acquire. KDE user suppression does not create an acquisition storm.
- GNOME flags, version/profile guard and normal manual/lid paths; Hypridle required setup versus acknowledged integration; KDE pending/active/suppressed distinctions.
- macOS FFI ownership, assertion release, process-exit cleanup, display-off/lock and resume.
- Windows request counters and handle/notification lifetimes, manual-sleep termination of requests, new fresh observation on resume, no unconditional renewal. Win+L alone must not cause our application to clear/recreate the request or add a display request. Already terminated requests must not prevent final handle cleanup.
- Callback context teardown cannot race a late callback; no callback blocks sleep or touches destroyed controller state.
- Normal exit, signal/console termination, forced kill, and a child that remains alive after its owner.

**Pass:** each native profile still passes its P0 matrix after integration; literal native call arguments and resource counts agree with intent; independent per-backend findings are verified/fixed. Record `docs/reviews/P4-<backend>.md` for `hypridle`, `gnome`, `kde`, `macos`, and `windows`.

## P5. Herdr popup, controls, and command behavior

Tests first:

- Model/render snapshots for every work/observation/native state and small terminal sizes; no invisible action when `ui_busy` or no UI client exists.
- Two popup clients change the same shared setting and see the acknowledged result. Closing/reopening the window leaves the monitor/native state unchanged.
- Keyboard navigation, resizing, terminal mode restoration on normal exit/error/panic, and sanitization of hostile control characters in external names/errors.
- Live Pause while working, persistent Pause after restart, immediate release on Pause-save failure with a visible warning, and no Resume acquisition when persistence fails.
- Monitor crash while the window is open; passive refresh remains read-only; explicit Retry can activate a new monitor.
- Run the actual `status --json` executable through stdout/exit-code tests, including stopped/hung/incompatible/permission-denied owners. A serializer-only test is insufficient.
- Count at least 100 status reads in a controlled replay: they advance `status_reads`, not Herdr snapshot origins or native acquire/release counts. Account separately for scheduled polls.

**Pass:** popup behavior verified in real Herdr, no ownership tied to UI lifetime, and the public command exactly matches its contract. Record `docs/reviews/P5-ui-api.md`.

## P6. Replays and end-to-end power acceptance

### Real traffic replay

Record approved actual Herdr workflows: working -> blocked, working -> done/idle, rapid successive work, two overlapping servers, client detach, monitor/server restart, and disconnect during work. Redact irrelevant fields while preserving field types, ordering and identity relationships. Store source version/commit, capture method, redaction rules and expected decisions beside each trace. Synthetic faults added to a trace must be labeled as injected faults, not attributed to real Herdr behavior.

Replay the recorded wire traffic through the real parser, scheduler/controller, public query projection and an instrumented backend. Check an explicit decision/resource ledger, not merely a final boolean. Then run approved equivalent workflows against actual Herdr and the real backend on each native profile; replay is not a substitute for those OS checks.

Required observable experiments:

| Experiment | Required counters/evidence |
| --- | --- |
| Many unchanged working snapshots | Exactly one acquisition until an actual release/loss; snapshot counters increase, native acquisition count does not. |
| Two servers overlap | A stopping does not release while B still works; one global native owner, deduplicated counts. |
| Repeated reads | Public status calls cause no Herdr refresh/native operation; their own counter alone reflects the calls apart from independently labeled scheduled work. |
| Hint burst | Hints are counted; requests are bounded/coalesced, including at most one dirty follow-up after an in-flight read. |
| Unknown/error repeats | Retention deadline does not move; work remains unknown after expiration; resource release is observed. |
| Recovery / old response | Old generation ignored counter advances; current snapshot is not rolled back; new request acquired only from fresh work. |
| Last work completes | Exactly one release after normal grace; no application-issued suspend call. Desktop policy resumes on its own terms. |
| Forced owner death with living child | OS inhibitor/assertion/request and singleton ownership disappear; no resource survives via an inherited descriptor. |

### Controlled native power experiment

Use an authorized test machine with saved work. Do not automatically suspend the user's workstation, close its lid, alter timers, or disable the installed `herdr-stay-awake`/other inhibitors. Those require a separate explicit test window/consent.

For each profile:

1. Record all existing inhibitors/assertions/power requests and idle/display/lock policy before the trial. Obtain permission to remove conflicting test influences, including the installed Stay Awake plugin. Restore agreed test changes afterward.
2. Establish a control trial without this application's request: the machine must actually follow its configured idle-sleep policy. Otherwise the protected trial cannot prove anything.
3. Establish real Herdr `working` evidence (not just an arbitrary live terminal), close the plugin popup, avoid input that resets idle, and observe the native request plus an independent workload progress counter. Leave the screen-off/lock policy unchanged unless the approved test setup explicitly requires a shorter timer.
4. Confirm that display-off/lock occur and the independent workload continues beyond the control idle-sleep point, except for the explicitly accepted Windows battery/model case.
5. End work. Confirm our resource is gone and the desktop resumes its own idle policy. The application must not request suspend or guarantee that the OS reuses versus resets an expired timer.
6. In a separate authorized trial with work reported, invoke normal explicit sleep or close the lid. Verify this app does not prevent the configured action, then resume and verify a new observation/request lifecycle. Do not infer lid behavior from manual sleep alone.
7. Repeat with Pause, backend restart, and forced owner termination. Preserve actual resource listings, system sleep/wake events, timestamps and workload-counter values.

Observation tools include `systemd-inhibit --list` / logind D-Bus and desktop-specific properties on Linux, `pmset -g assertions` plus sleep/wake logs on macOS, and `powercfg /requests` / Windows power event logs on Windows. Tools can require operator privileges for inspection; that does not authorize elevating the application. A listed handle/request is necessary evidence, not sufficient evidence of continued independent workload execution.

### Mandatory behavior matrix

| Profile | Required result / caveat |
| --- | --- |
| Hypridle 0.1.8 with documented listener setup | Idle host sleep inhibited; dim/off/lock unaffected; manual/lid policy unaffected; real release/kill cleanup. Without setup, actionable non-protecting state. |
| GNOME 51 qualified session/settings-daemon/systemd profile | Idle suspend/hibernate path blocked as requested; display/lock and ordinary explicit/lid actions preserved; no newly introduced strong sleep lock. |
| PowerDevil 6.7.5 | Pending during real activation delay; active only after service confirmation; user suppression respected; display/lock/manual/lid and cleanup correct. |
| macOS on each shipped architecture | Idle-system assertion only; independent work with display off/locked; explicit/lid actions and cleanup/resume correct. |
| Windows, traditional S3, AC and battery where supported | Ordinary idle request, display off/locked, independent work, manual/lid sleep, cleanup/resume. The Modern Standby exception does not apply here. |
| Windows, Modern Standby, AC | Same required behaviors including lock/display-off; do not apply the battery exception to AC. |
| Windows, Modern Standby, battery | Record actual behavior and the documented possible loss of protection; no workaround required or allowed. Verify normal request/cleanup and truthful status, not an unlimited-awake guarantee. |

Hardware without a particular sleep model can test its own row only. Mark other required rows blocked until an appropriate authorized machine supplies evidence. User-configured lid `do nothing`, disabled idle sleep, another inhibitor, or an active input simulator invalidates the corresponding counterfactual; it is not a pass.

**Pass:** recorded real-wire replay, real Herdr workflows, and native A/B evidence for every required profile/row, with only the already approved exception. Record `docs/validation/end-to-end.md` and `docs/reviews/P6-end-to-end.md`.

## P7. Packaging, regression suite, and independent final review

- Build/test the declared native target matrix using the pinned toolchain and lockfile. Run `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked` on relevant native hosts; target-specific unsafe/FFI modules must actually compile in their native jobs.
- Test source installation with Rust/Cargo, and prebuilt installation on a machine without them; the prebuilt manifest must omit source build steps. Test the external executable path without `HERDR_*` variables, first activation, live enable with already-working agents, Pause persistence, popup close, restart, documented update sequence and uninstall. An update that cannot replace a Windows executable must fail clearly, not leave a falsely reported new version.
- Verify minimum Herdr/version errors, unsupported desktop/setup diagnostics, custom endpoint registration, multi-session scope, and no runtime Node/Python/jq/power-management-wrapper dependency.
- Audit production native call sites against the allowed flag/method list. Search-based guard tests support, but do not replace, fresh source review.
- Independently review source, tests, fixture provenance, public JSON, resource cleanup, OS evidence and install documentation against the original contract. The reviewer must read current files rather than summaries. Main implementation ownership independently verifies each finding before fixing/reclassifying it.
- Findings records include requirement, severity, exact file/line or native artifact, reproduction, root cause, and regression test. Separate real defects from duplicates, unsupported assumptions and theoretical cases outside the deployment boundary. No unapproved scope-expanding fix.
- Every real defect is fixed and re-reviewed; mandatory matrix gaps remain blockers. Code review does not certify an unrun native test.
- Publication is a separate authorized action with license/identity/signing decisions settled. No automatic commit/tag/upload/marketplace submission.

**Pass:** all earlier gates still pass, required independent reviews are closed with evidence, documentation matches actual behavior, and the owner explicitly approves the phase/release commit. Record `docs/reviews/P7-final.md` and a release checklist.

## Requirement-to-evidence traceability

| Requirement | Primary checks |
| --- | --- |
| Only idle sleep; no lid/manual/display takeover | P0 native probes, P4 exact API arguments, P6 A/B/manual/lid matrix, P7 source audit. |
| Only reported working counts | P1 reducer, P3 parser/real Herdr scope, P6 captured workflow replay. |
| All current-user local Herdr servers | P2 singleton, P3 discovery/registration/two-server/headless tests, P6 overlap experiment. |
| Shared persistent Pause independent of popup | P1 persistence failures, P2 ownership, P5 multi-window/restart/close checks. |
| Read-only public status | P1 schema, P2 no-bootstrap check, P5 executable/stdout tests, P6 labeled I/O counters. |
| Bounded observation-loss holding while the controller runs / no false empty state | P1 deadlines, P3 transport faults, P4 resource loss, P6 nonrenewing unknown/recovery ledger. |
| Native cleanup, including abrupt failure | P0 kill, P2 inheritance, P4 per-backend cleanup, P6 independent resource listings. |
| Simple documented Windows exception | P0/P6 separate S3/Modern-Standby and AC/battery rows, P5 help/JSON, P7 no-workaround audit. |
| Correct first activation and update/uninstall | P0 lifecycle, P3 registry checks, P5 explicit Retry, P7 installation matrix. |

## Final exit conditions

- **PLAN COMPLETE:** architecture/contracts/defaults/evidence and all future gate procedures are written and consistent. This is the intended output of the current planning request, not implementation approval or release readiness.
- **IMPLEMENTATION BLOCKED:** identify the exact failed/missing gate and required evidence/decision; keep that phase incomplete. Do not substitute mocks, a stronger inhibitor, or a silent compatibility exception.
- **IMPLEMENTATION COMPLETE:** every mandatory gate and independent review has passed with recorded evidence. Only then can implementation be proposed for final acceptance; committing/publishing still needs explicit permission.
