# Mixed static-pressure MINI Stokes 2D

This case is the first execution of a nonzero field-valued fluid boundary.
The direct Model and an exact four-release package closure describe the same
three zero-velocity sides and one normal-pressure side. Both must lower and
execute without package dispatch.

The pressure boundary is Model meaning: a distinct volume Field is defined as
`4.5 Pa` and bound to `NormalPressureTraction2d`. The Realization supplies the
partial P1 trace elimination, MINI spaces, constant-traction facet operator,
and an empty algebraic-constraint list. It must not fabricate the previous
zero-integral pressure gauge.

The manufactured solution is `u=0` and `p=0.75 x + 1.5 Pa`. The nonzero
pressure integral, facet load, reaction, and three-way global balance jointly
falsify a stale gauge, a missing facet term, or a reversed normal/sign.

See [RFC 0047](../../../rfcs/0047-mixed-stokes-static-pressure.md).
