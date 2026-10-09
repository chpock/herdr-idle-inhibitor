## Review

**Scope:** current uncommitted Windows/macOS backends, clock, security, singleton, bootstrap, IPC, native-worker lifetime, and associated tests. Read-only review; supervisor fixes were freshly rechecked.

### Correct
- **Idle-only arguments:** macOS uses `PreventUserIdleSystemSleep`, level `1`, owned CoreFoundation strings, and balanced assertion release (`src/backend/macos.rs:20–61`). Windows sets only `PowerRequestSystemRequired`, with a fixed UTF-16 reason and version-0/simple-string context (`src/backend/windows.rs:16–40`).
- **Failure cleanup:** failed Windows acquisition drops its handle; release closes the handle even when `PowerClearRequest` fails (`windows.rs:31–36,49–74`). macOS retains its assertion ID after failed explicit release, allowing retry (`macos.rs:43–61`).
- **Callback lifetime:** callbacks access static atomics, not destroyed controller/heap context; registration has an unregister guard (`windows.rs:77–162`). Reviewed signatures against locked `windows-sys` bindings and Microsoft notification documentation.
- **Ownership boundary:** the worker serializes native operations and retains ownership after unconfirmed cleanup (`src/controller.rs:107–205`). Shutdown awaits worker cleanup before relinquishing the owner (`controller.rs:1041–1045`). Delayed acquisition and release-failure regressions cover this boundary (`controller.rs:1351–1502`).
- **Private IPC:** SID-based naming, current-user/SYSTEM DACL, peer-SID comparison, and exclusive persistent file locking are present (`src/runtime/paths.rs:43–58`; `windows_security.rs:26–104`; `src/herdr/transport.rs:60–73`; `singleton.rs:55–123`). Inspected Interprocess 2.4.4 defaults reject remote clients and create non-inheritable listener handles.
- **Detachment/freshness:** bootstrap redirects standard handles and uses platform detachment flags (`src/runtime/bootstrap.rs:23–57`). Suspend-aware clocks and generation invalidation prevent reuse of stale observations (`src/clock.rs:12–34`; `controller.rs:820–865`). No lock-specific workaround, privileged service, display request, or power-policy modification was introduced.

### Fixed — P1, supervisor correction independently rechecked

**Windows notification-registration failure silently enabled callback-less inhibition.**

- **Requirement:** implementation-plan 6.6 requires suspend-loss handling and fresh reconciliation; implementation-validation P4 requires notification lifecycle coverage.
- **Root cause/reproduction:** the original registration failure returned silently. Following user sleep shorter than the ten-second gap fallback, a terminated request could remain reported as accepted without being recreated.
- **Resolution:** registration now rejects nonzero results and null handles (`src/backend/windows.rs:113–144`); failure propagates through `src/backend/mod.rs:153–161`. Controller startup checks registration **before starting the native worker** (`src/controller.rs:1012–1013`).
- **Regression:** failed/null registration checks and explicit native unregister test (`windows.rs:166–181`); short suspend/resume test rejects old root/snapshot generations and requires fresh evidence before reacquisition (`controller.rs:1075–1165`). The latter asserts two acquisitions and one release.

No issues found.

### Validation and residual risks

- Supervisor reports **13 library tests passed**, formatting completed, and warning-free all-target cross-checks using matching Rust 1.98.0 libraries for `x86_64-pc-windows-gnu` and `aarch64-apple-darwin`. These commands were not independently executed by this reviewer.
- Fresh diagnostic inspection reported no errors/warnings, but Windows/macOS modules were inactive on this host; analysis-cache categories were incomplete. This is **not native compilation evidence**.
- Native CI remains **unrun**. Windows registration tests were cross-compiled, not executed. Opt-in native ownership/notification tests remain pending.
- MSVC linking, native detachment, ACL enforcement, owner-death/child inheritance, and real suspend/resume behavior remain unverified. Physical sleep/display/lock/lid/battery checks are owner-waived for this delivery, not passed.
- Planned CI configuration does not establish a passing CI run or complete P4 acceptance.

**Merge verdict: OK with notes for the reviewed source seam. Native validation remains pending; this is not release certification.**