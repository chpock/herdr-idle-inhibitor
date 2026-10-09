# Planning self-review

Status: specification corrections are recorded below and the documentation checks passed. This is an author self-review, not an independent implementation review. There is no application implementation to certify. The former Windows lock-screen planning blocker was withdrawn after API/source verification; ordinary native acceptance remains required.

## Scope and method

Freshly read the actual implementation plan, public status contract, validation sequence, baseline, scope record, glossary and architecture evidence. Rechecked the Herdr `AgentInfo` schema and manifest identifier/platform documentation. Compared state transitions, process/resource ownership, lifecycle, privacy, packaging and acceptance claims against those files.

The findings below concern the proposed specification. They are not reports of reproduced bugs in an existing application. Independent implementation reviews remain mandatory at P0–P7.

## Findings and corrections

### PS-01 — Mixed unknown state lacked an exhaustive release rule

- **Severity:** high specification gap.
- **Finding:** after A explicitly finishes, an unknown B that never reported work could leave an owned request without a clearly applicable release rule.
- **Correction:** only the already justified normal grace may finish; absent fresh work or a valid deadline, release. Unknown B cannot create or renew retention. See implementation-plan.md section 5.4.
- **Regression gate:** P1 tests A ending while B is unknown, cold unknown state, expired retention, and no reacquisition merely for grace.

### PS-02 — Wake could expire lifecycle metadata before revalidation

- **Severity:** high lifecycle gap.
- **Finding:** suspend-aware elapsed time correctly expires work evidence but can also make every pre-suspend discovery/eligibility timestamp look old enough to exit immediately after wake.
- **Correction:** release expired ownership, invalidate pre-gap requests, and permit one bounded revalidation round before age-based shutdown. This does not renew inhibition or reuse stale work. See section 5.3.
- **Regression gate:** P1/P3 long-suspend tests with both successful revalidation and real post-wake failures.

### PS-03 — Retained installation eligibility needed a separate bound

- **Severity:** medium permission/lifecycle gap.
- **Finding:** temporary uncertainty about plugin enablement could otherwise be mistaken for permission to acquire a new request or renew an old authorization forever.
- **Correction:** existing ownership only, thirty seconds from last verified enablement; no renewal on failures or reacquisition after native loss. Work and eligibility deadlines independently constrain holding. See sections 4.2 and 5.4.
- **Regression gate:** P3 registry failures, native loss during retained eligibility, and multiple roots with one still freshly enabled.

### PS-04 — Public names and EOF outcomes were ambiguous

- **Severity:** medium API-contract gap.
- **Finding:** a count named `servers_running` included uncertain disconnected endpoints; the unavailable exit description also overlapped partial-frame protocol errors.
- **Correction:** use `servers_tracked` with explicit semantics. Empty EOF is unavailable; partial/malformed/oversized data is a protocol failure. Counter origins/outcomes are also defined explicitly in status-api.md.
- **Regression gate:** P1 JSON/schema cases and P5 executable stdout/exit-code tests.

### PS-05 — Minimal IPC did not alone prevent metadata retention

- **Severity:** medium privacy gap.
- **Finding:** child environment inheritance could preserve full hook context even when the registration message was minimal. Herdr's real `AgentInfo` schema also includes metadata we do not need.
- **Correction:** strip full context/event payload variables when spawning the owner. Parse only terminal identity/status and discard raw frames; never claim optional metadata was not received at all. Do not call terminal-output APIs. See sections 3.1, 5.1 and evidence H4.
- **Regression gate:** P2 child-environment/log checks and parser fixtures carrying ignored metadata.

### PS-06 — A prebuilt bundle must not retain Cargo build hooks

- **Severity:** medium packaging gap.
- **Finding:** shipping a binary alongside an unchanged source manifest can still invoke a Cargo build during installation.
- **Correction:** the prebuilt manifest omits source build steps while preserving entrypoints/metadata. Document the executable path; plugin installation does not implicitly add a PATH command. See section 10.
- **Regression gate:** P7 prebuilt installation without Rust/Cargo and external status invocation without Herdr environment variables.

### PS-07 — Controller deadlines are not kernel-enforced leases

- **Severity:** medium reliability-claim gap.
- **Finding:** bounded observation-error retention could be read as a guarantee that a completely frozen process releases its resource on schedule.
- **Correction:** distinguish termination from suspension/hang. A frozen owner can retain its OS resource until execution resumes or it is terminated; no hidden watchdog is proposed. Deadlines initiate release but do not prove asynchronous cleanup finished. See sections 1.2 and 5.3.
- **Regression gate:** P3 distinguishes a stopped process from a killed process; P0/P4/P6 prove actual termination cleanup independently.

### PS-08 — API selection and implementation verification were conflated

- **Severity:** high verification-process gap.
- **Finding:** the draft first closed all planning topics, then classified Windows locking as a factual architecture blocker based on PowerToys help without first checking the selected API and Awake implementation deeply enough. Lack of an executed acceptance test is not by itself evidence that an API cannot meet its documented purpose.
- **Correction after the user's documentation request:** direct PowerSetRequest documentation names no lock-only cancellation; checked Awake source uses SetThreadExecutionState and preserves ES_SYSTEM_REQUIRED when locked. The product-help warning is insufficient to impose a restriction on our API. Withdraw the special U01 planning blocker while retaining ordinary native acceptance. See [evidence W2](../research/architecture-evidence.md#w2-windows-session-locking-and-power-requests).
- **Verification:** planning completion means a source-backed API choice, not an unconditional hardware guarantee. P0 and the integrated matrix still test Win+L and a separate workload; no such test has run and no extra Windows exception was approved.

### PS-09 — Empty initial scope needed an explicit unknown state

- **Severity:** medium state-contract gap.
- **Finding:** completeness over an empty set is ambiguous before registration/discovery establishes scope; discovery/eligibility freshness also needed a bound separate from agent snapshots.
- **Correction:** cold/unestablished scope and newly registered unverified roots remain incomplete. Discovery/eligibility freshness is ten seconds, with immediate uncertainty on failed checks. No empty-set shortcut establishes `none` at startup.
- **Regression gate:** P1 startup, new-root, independent-age and public projection tests.

## Verification record

- The disposable validator at `/tmp/herdr-idle-inhibitor-plan-check.py` checks the twelve English Markdown artifacts, local links/anchors, balanced fences, final newlines, source catalog JSON, synthetic status JSON, proposed TOML defaults and P0–P7 coverage.
- The first check found a missing final newline in this report. A later actual-file read also exposed a truncated report body, which structural checks alone had missed. The report was rewritten fully; the checker was strengthened and first demonstrated rejection of truncated artifact content. The strengthened rerun passed across all twelve documents, including 56 local links/anchors and both structured examples; no application/native test result is implied.
- `git diff --check` passed during review. Untracked documents require the explicit validator too; git's tracked diff alone does not cover them.
- `aft_inspect` completed a fresh inspection but has no authoritative diagnostics for some Markdown/JSON files and lacks some Tier-2 categories. This is not a compiler or native-test pass.
- No Rust application, real native power experiment, real traffic replay, or independent implementation review was run. Their future procedures and stop rules are in [implementation-validation.md](../implementation-validation.md).

## Review exit

The corrections above address specification findings only. The [plan](../implementation-plan.md) is ready for review; [U01](../grill-session.md) is resolved at API-selection level after the documentation/source check, and native implementation acceptance remains unrun. The earlier special lock-screen blocker was an unsupported classification, not an observed product defect. Implementation, commits and publication remain unapproved.
