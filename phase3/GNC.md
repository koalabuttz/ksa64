# Phase 3 flight software

The flight computer implements `Boot`, `Prelaunch`, `ProgrammedAscent`,
`StageTransition`, `Insertion`, `Coast`, `Complete`, and latched `Abort` modes.
It owns engine and separation requests. The world retains authority to reject
commands that violate burn, ignition, or separation constraints.

The KSA-2A pitch program remains trusted through early ascent. Sixteen steps
(two seconds) after confirmed second-stage ignition, guidance enters insertion.
From T+220 s it regulates target radius/tangential energy and angular-momentum
proxies around prograde while damping radial velocity. Commands remain inside
the 70-110 degree insertion envelope. Estimated orbit readiness triggers upper
cutoff; a flight-software time guard commands cutoff one step before the KSC2
physical maximum-burn guard.

During insertion, a command-versus-feedback pitch error above two degrees for
16 consecutive steps latches abort. Abort commands cutoff, inhibits later
ignition and separation, sets safeing, and sets the transport-only recovery
request while the vehicle continues ballistically.

Malformed sensor transport also latches this fail-closed state.
