# PowerDevil suppressed-request ownership blocker

Status: independently identified during P4, verified against fresh pinned source, and explicitly accepted by the owner. KDE support is retained with this documented exception. No physical KDE reproduction has been performed. This is not a Windows/macOS hardware-access gap.

## Requirement

Implementation plan sections 2 and 6.1 require the native request to cease belonging to a terminated monitor, including abrupt termination. Section 6.4 requires respecting user suppression and retaining one cookie rather than repeatedly acquiring. PowerDevil 6.7.5 was the proposed qualified KDE profile.

## Authoritative source

PowerDevil source revision `5627bacf05394a737f1dd50295b187748150ffb1`, inspected in `/tmp/herdr-idle-inhibitor-research/powerdevil-grill/daemon/powerdevilpolicyagent.cpp`:

- [Acquisition and suppression, lines 608–683](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L608-683): acquisition associates the cookie with the sender's unique D-Bus service. Suppression calls `ReleaseInhibition(cookie, true)`; later allowance reactivates the retained cookie.
- [Release, lines 733–758](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L733-758): owner mapping and service watcher are removed unconditionally, before the `retainCookie` test. The requested cookie, inhibition metadata, and allowance callback remain when retaining.
- [Owner disappearance, lines 516–526](https://invent.kde.org/plasma/powerdevil/-/blob/5627bacf05394a737f1dd50295b187748150ffb1/daemon/powerdevilpolicyagent.cpp#L516-526): cleanup enumerates only cookies still in the owner map.

## Reachable transition

| Transition | Requested cookie | Active | Owner association |
| --- | --- | --- | --- |
| Acquire and complete activation delay | Present | Yes | Present |
| User suppresses the request | Present | No | Removed |
| Monitor dies / its sole D-Bus connection closes | Still present | No | Already absent; exit cleanup cannot find the cookie |
| User allows the same application/reason | Still present | Yes again | Not restored |

Consequently an orphaned request can become active after its requesting process is gone. A Rust destructor does not run after a kill; closing the dedicated connection cannot repair a service that has discarded its owner association. Explicit graceful cookie release is different and can still work.

## Separate implementation defect corrected

The five-second pending interval precedes insertion into `RequestedInhibitions`. Therefore initially empty requested/active properties are pending, not evidence of native loss. The implementation is being corrected to remember first confirmation, retain its cookie while initially pending, and recognize disappearance only after confirmation. This correction does not repair the upstream owner-lifetime defect.

## Owner-approved disposition

Do not silently waive process-exit cleanup, use random application/reason identities, add a watchdog/helper, or substitute a stronger sleep/display lock. The owner explicitly chose **accept the KDE limitation** in the implementation questionnaire: retain PowerDevil 6.7.5 support and document precisely the suppression/owner-death/reallow orphan scenario in README. This is an exception to guaranteed owner-death cleanup, not a blanket waiver of cleanup or permission to alter desktop power policy. The alternative of rejecting KDE was recommended and declined. A separately source-qualified fixed release can remove this exception in future.

Web search located related reports, but search-generated interpretations were not used as proof. The source paths above are the basis of this finding.
