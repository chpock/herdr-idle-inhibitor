## Review

**Scope:** current uncommitted runtime, Herdr transport/discovery, controller, diagnostics, and associated tests. Native adapters, UI, and packaging are not approved by this review.

### Correct
- Singleton ownership precedes stale-socket reclamation; the lock inode survives shutdown (`src/runtime/singleton.rs:23–42`).
- IPC authenticates peer identity, bounds frames/concurrency, and enforces deadlines (`src/herdr/transport.rs:20–31`; `src/runtime/ipc.rs:24–58`).
- Status requests do not evaluate policy or schedule I/O (`src/controller.rs:77–91`). Executable and integration tests check query counters and native-operation counts.
- Discovery health and installation eligibility have separate evidence and freshness tracking (`src/controller.rs:46–51,133–143`).

### Fixed — supervisor changes independently rechecked

| Finding | Requirement and root cause | Verified resolution and regression |
|---|---|---|
| **P1: Cross-root environment leakage** | Plan 4.1 requires each registered root’s discovery context. Environment overlays retained the owner’s absent-in-registration XDG overrides. | Root-selection variables are explicitly removed before applying registration (`src/herdr/discovery.rs:28–33`). Regression: `tests/protocol.rs:20–28`. |
| **P1: Custom-endpoint handoff loses eligibility** | Plan 4.1 requires registered endpoints alongside discovered endpoints. Eligibility previously remained tied to the first registration after that server disappeared. | Registered endpoints now survive empty discovery; eligibility tries same-root candidates, prioritizing current observations (`src/controller.rs:69–72,99–112`). Real-transport regression checks registry calls on surviving endpoint B (`tests/integration.rs:47–49`). |
| **P1: Endpoint aliases double-count agents** | Plan 4.1 requires normalized native endpoint identity. Configured endpoints previously bypassed normalization. | Configured insertion and retirement relevance now normalize identities (`src/controller.rs:94,191`). Regression verifies one server and one scheduled snapshot for alias/canonical registrations (`src/controller.rs:242–247`). |
| **P1: Queued pre-suspend results become fresh evidence** | Plan 5.3 requires invalidating pre-gap generations before using results. Previously only the timer branch detected execution gaps. | Results and mutating controls detect gaps before application; status remains read-only (`src/controller.rs:77–80,130–131,163–169`). Deterministic regressions cover old queued results and query purity (`src/controller.rs:249–260`). |

No issues found.

### Validation and residual gaps
- Supervisor reports `cargo test --locked --lib --test protocol --test integration --test runtime --test cli` passed: **4 + 3 + 1 + 3 + 1 tests**. Test implementations were freshly inspected; commands were not independently executed by this read-only reviewer.
- Both scoped diagnostic inspections were interrupted; neither produced a fresh health result.
- Native Windows pipe ACL/credential behavior, Windows/macOS process lifetime, and owner-death-with-living-child resource inheritance remain unverified here.
- Current tests do not establish the complete planned hint-burst, deadline, transport-fault, and lifecycle matrix.
- Synthetic local-transport integration is not recorded real Herdr traffic or physical sleep/lock/lid acceptance.

**Merge verdict: OK with notes for the reviewed seam.** This does not certify completion of every P2/P3 validation gate or platform support.