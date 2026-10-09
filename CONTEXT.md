# Herdr idle inhibition

This project observes work reported by local Herdr servers and temporarily inhibits idle-triggered host sleep. It is not a lid controller or a sleep scheduler.

## Language

**Idle sleep**: Host sleep initiated because of user inactivity under the OS or desktop power policy. Display-off and screen locking are separate behaviors.
_Avoid_: "all sleep", "screen idle", "lid sleep" as interchangeable names.

**Manual sleep**: Sleep explicitly requested by the user, rather than by an inactivity timer. This plugin does not block that request.

**Herdr server**: One local Herdr server/API endpoint with its workspaces and panes, including a named or default session.
_Avoid_: "session" without saying whether it means a server or an agent conversation.

**Working agent**: An agent whose current Herdr-reported status is exactly `working`. An open process, a blocked agent, or a detached task not represented by Herdr is not independent proof of this state.

**Observation health**: Whether the plugin has sufficiently current, usable information about its relevant Herdr servers and agents. Missing or invalid information is not a report that no work exists.

**Observation coverage**: The declared local roots and endpoints over which observation health is evaluated. Complete coverage of that declared set is not proof that an arbitrary unregistered custom server cannot exist.

**Desired inhibition**: The decision to hold idle-sleep protection given observed work, shared controls, and the documented timing/error policy. It is not proof of resource acquisition.

**Inhibition resource**: An OS/desktop resource owned by the running monitor that requests suppression of idle sleep. Resource ownership is distinct from a guarantee that the OS cannot sleep for another reason.
_Avoid_: A persisted "active" flag as proof of protection.

**Accepted native request**: A request acknowledged by the selected OS/desktop mechanism. It is neither a work observation nor a guarantee that the OS cannot override the request.

**Pause**: A user control that stops this plugin's idle inhibition while retaining observation. Its scope is all monitored local Herdr servers of the current OS user; the word does not imply a timed automatic restart.

**Release**: Relinquishing this plugin's own inhibition resource. It neither issues a suspend command nor promises that the OS will immediately go to sleep.

**External status query**: A one-shot read-only request for the plugin's current reported state. It is not an event subscription or an instruction to launch another application's power policy.
