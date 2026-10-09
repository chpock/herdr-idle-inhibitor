## Review

**Scope:** P1 model, policy, clock, configuration, status projection and tests. Requirements freshly checked against implementation-plan 7 and status-api.md. Runtime/backend/UI completeness was excluded.

### Correct
- Atomic snapshot validation, terminal deduplication and unknown aggregation: `src/model.rs:24–35,50–65`.
- Pause applies before persistence; Resume changes live state only after successful saving. External edits prevent overwrite: `src/runtime/config.rs:47–64`.
- Suspend-aware native clock selection: `src/clock.rs:2–22`.

### Fixed by parent; independently rechecked
1. **P1 — Retention expiry manufactured normal grace.**
   Requirement: nonrenewing observation retention, implementation-plan 5.4. An unrelated server’s earlier completion was processed after another server’s retention expired.
   **Resolution:** consume completion events during retention and anchor normal grace to completion time: `src/policy.rs:23–34`. Regression covers 5/60-second grace and 30/120-second evaluation gaps: `tests/policy.rs:90–100`.

2. **P1 — Expired unknown evidence hid confirmed completion.**
   Requirement: normal grace when the last retainable work explicitly ends, implementation-plan 5.4. Expired positive entries prevented the server’s positive map becoming empty.
   **Resolution:** expiry-aware evidence bookkeeping: `src/model.rs:30–33`. Same-server, two-agent regression verifies completion at 41 seconds: `tests/policy.rs:102–110`.

3. **P1 — Public contract accepted missing fields and incompatible shapes.**
   Requirement: status-api.md required fields, fixed enumerations and unavailable nullability. Permissive DTO deserialization accepted missing nullable keys, unsupported values and unavailable-as-zero observations.
   **Resolution:** explicit wire validation and constrained schema: `src/status.rs:37–88`; `schemas/status-v1.json:377–500`. Positive/negative boundary and schema tests: `tests/status.rs:3–42`.

**Current findings:** No issues found.

**Merge verdict: OK with notes — designated P1 seam only.**

### Residual validation gaps
- Reviewer could not execute shell commands. Supervisor reports passing focused tests; this is not independently executed test evidence.
- Fresh diagnostics reported zero errors/warnings across all eight scoped Rust files. Other analysis categories remained incomplete because of cache contention.
- Windows/macOS execution, physical power behavior and eventual runtime/CLI integration remain unverified here.