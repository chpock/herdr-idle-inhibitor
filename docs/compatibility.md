# Compatibility and limitations

[Home](../README.md) · [Advanced installation](advanced-installation.md) · [Troubleshooting](troubleshooting.md)

## Herdr and deployment scope

The supported Herdr API version is **0.9.3**. Older/newer versions are rejected until supported explicitly. `min_herdr_version` in the plugin manifest is an installation minimum, not a promise that every subsequent API is compatible.

Run as the current user on the native host, with one active OS desktop session and any number of registered local Herdr servers. Remote-host control, WSL-to-Windows/container-to-host power bridges, other users' agents, and concurrent mixed-desktop/multi-seat sessions are outside this deployment scope.

## Platforms

| Platform | Architecture/profile | Native request |
| --- | --- | --- |
| Linux GNU | x86_64; Hyprland with Hypridle **0.1.8** | logind `idle / block`; optional [display/lock tuning](hypridle-setup.md) |
| Linux GNU | x86_64; GNOME session **51.0** | SessionManager suspend flag `4`, without idle/display flag `8` |
| Linux GNU | x86_64; PowerDevil **6.7.5** | PolicyAgent `InterruptSession = 1`, without screen policy |
| macOS | Apple Silicon / Intel; deployment target **13.0** | IOKit `PreventUserIdleSystemSleep` |
| Windows | x86_64 MSVC; Windows **10+** | `PowerRequestSystemRequired`, without Display/Away Mode |

Linux checks the selected desktop program's version and rejects unqualified profiles; manually selecting it does not bypass the guard. GNOME behavior is version-sensitive: the GNOME 51 suspend-only path uses weak logind forwarding and skips inhibitors for explicit sleep, unlike some older implementations. Distribution patches and settings still matter.

Linux binaries are built on Ubuntu 24.04 with **glibc 2.39**; older libc compatibility is not promised. There are no Linux/Windows arm64 artifacts. Both macOS architectures are native binaries, not a universal archive.

Local workstation checks were performed on Linux. Native hosted builds, unit/integration tests, actual Windows/macOS API lifecycle and archive execution passed on Ubuntu 24.04, macOS **15.7.9** for both architectures, and Windows Server 2022. macOS 13 and Windows 10 are runtime targets, not those hosted execution environments. **Physical idle-sleep, display/lock, lid and hardware resume behavior, installed-Herdr lifecycle and clean-machine security prompts have not been certified.** GNOME/KDE checks use isolated D-Bus interfaces rather than physical desktops. Please report platform failures as [issues](https://github.com/chpock/herdr-idle-inhibitor/issues).

Automatic-update process tests currently run on Linux. The new Windows replacement-lock test and macOS update bindings have been type-checked, but native hosted checks for this update mechanism have not yet run. This is separate from the previously verified native power API lifecycle.

## What a native request means

The plugin requests only automatic idle-sleep prevention. It does not explicitly request that the display stay on, intercept lid/manual sleep, emit input, force suspend when work ends, alter a power plan or install a privileged service. Hypridle can also delay display-off/locking in response to an idle inhibitor; [optional listener tuning](troubleshooting.md#linux-screen-does-not-turn-off) separates those actions without blocking use of the plugin.

`resource_owned` and `request_state = accepted` are not an unconditional awake guarantee. The OS can honor safety policy, user actions or independent power settings. Unknown/setup/error/suppressed states are visible rather than hidden by a stronger fallback.

## KDE activation and suppression

PowerDevil has an approximately **five-second activation delay**. The plugin owns a cookie before the desktop reports it active and shows `pending` during that interval; it does not bridge the delay with a stronger request.

If you suppress the application in KDE, the plugin reports `suppressed` and keeps the existing cookie state. It does not repeatedly reacquire or change identity to defeat your choice. Later allowance can restore the request while the monitor is alive.

### PowerDevil suppressed-request cleanup

PowerDevil **6.7.5** can lose the owner association of a user-suppressed request while retaining its cookie:

1. The plugin holds an active request.
2. You suppress it in KDE.
3. The monitor dies or its connection closes while it is suppressed.
4. You later allow that application/reason again.
5. PowerDevil can reactivate an orphaned request although its monitor no longer exists.

Normal graceful explicit release is different and is still attempted. Closing a connection cannot repair owner tracking already discarded by PowerDevil. An orphan may require desktop-side cleanup by the user; this plugin does not release other applications' requests or restart PowerDevil. This specific exception is not a blanket failure of all process-exit cleanup.

[PowerDevil's retained-cookie release/owner mapping](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L733-758) and [owner-disappearance handling](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L516-526) describe this limitation. The status identifier `kde_powerdevil_suppressed_owner_cleanup` is a conditional caveat, not detection of an orphan.

## Windows Modern Standby on battery

On Modern Standby hardware running on battery, Windows can terminate ordinary system/execution power requests **five minutes after the system sleep timeout expires**. This is not five minutes after an agent starts and is not the Battery Saver toggle. The plugin may therefore fail to prevent idle sleep in that combination.

Independent Windows policy can also ignore application system-required requests. The application respects it: no power-plan rewrite, synthetic activity, special helper, Away Mode or model-specific workaround is used. Locking the display does not itself cause the plugin to clear or recreate the request.

Microsoft documents the conditions in [PowerSetRequest](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest) and [Modern Standby preparation](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/prepare-software-for-modern-standby). The status identifier `windows_modern_standby_battery` does not detect the current hardware or power source.

## Crash, freeze and recovery

Native resources normally disappear after their owner terminates, subject to the precise PowerDevil exception above. A frozen/stopped process is not a dead process and can keep a request; application deadlines cannot execute while it is stopped.

Herdr does not supervise the detached monitor. A later startup/event/action can reactivate it, but no finite restart guarantee exists without another event. Fresh observation is required after sleep/recovery; the plugin does not restore remembered work from disk. See [How it works](behavior.md#timing-and-recovery).

## Distribution and OS security

Prebuilt bundles are unsigned; macOS archives are also unnotarized. Gatekeeper/quarantine or Windows SmartScreen may prevent execution. Approve an individual application only when you trust its origin; do not disable system-wide protections. CI archive execution is not a clean-machine approval or installation guarantee.

A SHA-256 companion checks archive integrity but is not a signature. CI artifacts have limited retention and are not automatically promoted to published releases. Ordinary [GitHub installation](../README.md#get-started) builds from source. To build your own checkout instead, see the [development toolchain](development.md#toolchain).
