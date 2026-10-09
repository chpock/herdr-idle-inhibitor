# Lid-close and manual-sleep inhibition: feasibility

Status: historical feasibility research for a **superseded feature proposal**. The user subsequently excluded lid-close and manual-sleep control from this project; the [current baseline](../baseline-plan.md) is idle-only. See the [scope decision and rationale](../scope-decisions.md). This is not the grill, an implementation decision, or runtime acceptance. The Windows Modern Standby idle-inhibition limitation below remains relevant to the active scope.

## Original requirement (superseded)

At the time of this research, the requested behavior while Herdr reported working agents was:

- The default inhibits **idle-triggered host sleep only**. Display-off, screen locking, lid-close sleep, and manual sleep are not intentionally blocked by the default.
- An optional setting must provide lid-close sleep inhibition.
- Prefer to preserve manual sleep when lid-close inhibition is enabled.
- If the platform cannot provide the needed separation, the user accepts a **configurable combined lid-close/manual-sleep block**. This is an explicit fallback, not permission to block manual sleep by default or silently broaden a lid-only request.
- There is no independently requested "block manual sleep only" feature.

Permission to research these behaviors does not authorize changing the machine's power settings or installing a privileged helper.

## Capability summary

| Platform / mechanism | Can distinguish lid-close from manual sleep? | Ownership and limitation |
| --- | --- | --- |
| Linux, lid handled by systemd-logind | **Yes:** `handle-lid-switch` disables logind's lid handling without inhibiting an explicit `systemctl suspend`. | A file-descriptor-owned inhibitor; released on close/process exit. It does not intercept independent lid handling by a desktop environment or a custom script. |
| Windows, standard power request | **No lid-close guarantee:** ordinary System/Execution requests do not override lid-close or explicit user sleep. | Appropriate starting point for idle inhibition, not a lid-control mechanism. |
| Windows, lid action in a power plan | **Yes:** `LIDACTION = Do Nothing` changes lid behavior without changing the manual Sleep command. | A power-policy setting, not a process-owned inhibitor. AC/DC values and the applicable scheme must be handled explicitly. Restoration after failure and coexistence with user changes are unresolved implementation requirements. |
| macOS, public idle-sleep assertion | **No:** Apple explicitly permits lid-close and Apple-menu sleep while it is held. | Appropriate for the default; does not satisfy the optional lid policy. |
| macOS, lid-aware internal assertion property | A distinct mechanism exists in Apple's code, but creation of `AppliesOnLidClose` assertions is entitlement-gated. | Not established as an available public API for an ordinary third-party plugin; do not assume that root or an arbitrary signature grants this entitlement. |
| macOS, global `SleepDisabled` setting | **Combined behavior:** the inspected kernel path blocks both idle and demand sleep, including lid and software requests. | Requires privileged setting changes; global, persistent configuration rather than a per-client resource. A candidate for the accepted fallback, not an approved or crash-safe solution by itself. |

These are source-backed mechanism findings. No lid was closed and no sleep or power-policy experiment was performed on any of the three platforms.

## Linux evidence

The [systemd inhibitor documentation](https://systemd.io/INHIBITOR_LOCKS/#taking-key-handling-locks) explicitly says that key-handling locks disable logind's low-level event handling and have no effect on suspend requested through other mechanisms, giving `systemctl suspend` as an example.

A lid lock can therefore be separate from a broad `sleep` inhibitor. The lock is held by the file descriptor returned from `Inhibit`; closing it, including through process exit, releases it. Access is governed by logind's authorization policy.

**Boundary:** this is a guarantee about logind's handler, not every Linux desktop. If the desktop already handles lid events itself, taking another `handle-lid-switch` lock does not disable that desktop's code. Desktop integration must be verified for the chosen support matrix. The earlier Hypridle idle-versus-display distinction remains relevant to the default policy and is not solved by a lid lock.

## Windows evidence

### Idle inhibition is not lid inhibition

Microsoft's [PowerSetRequest documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest) states:

> Except for PowerRequestAwayModeRequired on Traditional Sleep (S3) systems, power requests are terminated upon user-initiated system sleep entry (power button, lid close or selecting Sleep from the Start menu).

It also documents that Away Mode changes explicit sleep into a state where processing continues with audio/video off, and that this request applies only to traditional S3 systems. [SetThreadExecutionState guidance](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setthreadexecutionstate) reserves Away Mode for narrowly justified media/background scenarios and says portable applications should not enable it. This is not an appropriate universal laptop fallback for this plugin.

### The lid action is independently configurable

Microsoft documents [Lid switch close action](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/power-button-and-lid-settings-lid-switch-close-action), alias `LIDACTION`, GUID `5ca83367-6e45-459f-a27b-476b1d01c936`. Value `0` means **Do Nothing**. This setting concerns lid closure, not the manual Sleep command.

[PowerWriteACValueIndex](https://learn.microsoft.com/en-us/windows/win32/api/powersetting/nf-powersetting-powerwriteacvalueindex) writes a setting in a specified power scheme. The documentation says changes to the active scheme do not take effect until `PowerSetActiveScheme` is called. Battery and plugged-in policy must be considered separately.

Consequently, separation is possible at the OS configuration level. It is not achieved by choosing a different ordinary idle-inhibitor flag. A plugin that changes this policy needs an explicitly approved ownership/restoration design; separate Herdr monitors cannot independently save, overwrite, and restore the same setting as if it were a reference-counted lock.

### Separate default-policy limitation

The same PowerSetRequest documentation says that on **Modern Standby systems on battery**, System/Execution Required requests are terminated **five minutes after the system sleep timeout expires**. A successful API return is therefore not proof of indefinite work protection on every Windows machine. Mandatory Windows support remains a user requirement; its concrete hardware/power-state coverage must be resolved rather than silently narrowed.

## macOS evidence

### Public idle assertions deliberately leave lid and manual sleep alone

Apple's [PreventUserIdleSystemSleep documentation](https://developer.apple.com/documentation/iokit/kiopmassertiontypepreventuseridlesystemsleep) states that the display may dim and sleep, but the system may still sleep for lid close, the Apple menu, low battery, or other reasons. The text was retrieved from Apple's [documentation data endpoint](https://developer.apple.com/tutorials/data/documentation/iokit/kiopmassertiontypepreventuseridlesystemsleep.json), because the rendered page requires JavaScript.

Apple's [PreventSystemSleep documentation](https://developer.apple.com/documentation/iokit/kiopmassertiontypepreventsystemsleep) describes a preference for remaining in Dark Wake, not a portable promise that every arbitrary user process and every lid scenario stays fully operational. Its name alone is insufficient evidence for the optional mode.

### A lid-specific internal path exists, but is gated

In Apple's PowerManagement commit `d415e45501842834a280930c3eed9186544a67f0`:

- [`PMAssertions.c`, `setClamshellSleepState`](https://github.com/apple-oss-distributions/PowerManagement/blob/d415e45501842834a280930c3eed9186544a67f0/pmconfigd/PMAssertions.c#L1389-L1467) handles assertions carrying `kAssertionLidStateModifier` separately from ordinary idle assertions.
- [`callerIsEntitledToAssertion`](https://github.com/apple-oss-distributions/PowerManagement/blob/d415e45501842834a280930c3eed9186544a67f0/pmconfigd/PMAssertions.c#L2536-L2591) rejects `kIOPMAssertionAppliesOnLidClose` when the caller lacks `kIOPMAssertOnLidCloseEntitlement`.

This rules out treating a hidden property name as an automatically usable public mechanism. It does **not** prove that every conceivable macOS-specific technique is impossible. No supported general-purpose lid-only API available to this plugin was established in this bounded check.

### The global fallback also blocks manual sleep

Apple's [`pmset.m`](https://github.com/apple-oss-distributions/PowerManagement/blob/d415e45501842834a280930c3eed9186544a67f0/pmset/pmset.m#L5813-L5836) translates `disablesleep` into `kIOPMSleepDisabledKey`. Its [settings application code](https://github.com/apple-oss-distributions/PowerManagement/blob/d415e45501842834a280930c3eed9186544a67f0/pmset/pmset.m#L779-L815) says the settings are written to disk, and reports when root privileges are required.

In Apple's XNU commit `f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`, [`IOPMrootDomain.cpp`](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/iokit/Kernel/IOPMrootDomain.cpp) connects `SleepDisabled` to `userDisabledAllSleep`. `checkSystemSleepAllowed` checks that flag under the comment "Conditions that prevent idle and demand system sleep". Both software and clamshell requests reach `privateSleepSystem`, which rejects sleep when `checkSystemSleepEnabled` fails.

The existing [Espresso helper](https://github.com/Hanyang-Li/espresso/tree/f7436781d93e93a8685e53aeb98d2c813ef552a6) demonstrates the same approach: [`src/power.rs`](https://github.com/Hanyang-Li/espresso/blob/f7436781d93e93a8685e53aeb98d2c813ef552a6/src/power.rs) writes the global setting through `IOPMSetSystemPowerSetting`. Its README explicitly warns that `SleepDisabled` has **no owner**, uses last-writer-wins behavior, and can overwrite another application's or the user's setting.

This supports the existence of a combined lid/manual fallback. It does not establish safe restoration after crashes, harmless coexistence, or exclusion of every emergency-sleep path. Those are conditions to resolve before selecting this backend. A global sleep kill switch is not equivalent to a resource that the OS releases when its owner exits.

## Historical consequences for the former baseline

These recommendations applied to the former optional-lid proposal. They do not reintroduce that proposal into the current idle-only contract.

1. Keep the default unchanged: idle sleep only.
2. Make optional lid protection visible as a separate capability, including its actual manual-sleep effect.
3. Never turn a requested lid-only mode into a combined mode without explicit configuration allowing the combined behavior.
4. Do not claim equal implementation mechanics across platforms: Linux can use a scoped resource in the logind case; Windows may need power-policy changes; the established macOS fallback is a privileged global setting.
5. Permission for the product feature is not blanket approval to install services, change global preferences, or adopt private APIs. Present those requirements before implementation.
6. Do not claim default or optional modes supported merely because native calls compile or return success. Runtime acceptance must include real lid/manual-sleep behavior, cleanup, AC/DC conditions, and relevant OS power models.

## Prior-art description correction

The earlier survey called `herdr-espresso` a Swift daemon. The pinned Herdr plugin is a **Rust watcher** calling the external Espresso CLI; the newly pinned Espresso source is also Rust. The survey entry has been corrected. This does not change the lid/manual-sleep findings above.
