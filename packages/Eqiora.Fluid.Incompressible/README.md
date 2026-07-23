# Eqiora.Fluid.Incompressible

This package owns one reusable steady incompressible Newtonian volume law and
one separate complete-exterior mechanical interface. The volume Component
binds root-owned velocity, pressure, and conservative force-potential Fields.
The boundary Component binds velocity trace and parent-outward Cauchy traction
to exact `Eqiora.Mechanics.Interfaces::VelocityTractionBoundary` Ports.

The Components expand into ordinary typed Relations. They own no Domain,
Field, boundary data, mesh, mixed element, quadrature, pressure constraint,
scaling profile, solver, target, or schedule. An enclosing Model connects each
boundary Port to an explicit terminal or another compatible physical Port;
Realization remains independent of package identity.

Version `0.2.0` does not claim nonzero boundary data, natural/open-boundary
execution, live trace transfer, transient flow, Navier--Stokes, moving geometry,
ALE, structural dynamics, or FSI. In particular, it never treats the distinct
quasistatic displacement/traction Connector as a velocity/traction Connector.
