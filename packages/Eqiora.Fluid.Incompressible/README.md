# Eqiora.Fluid.Incompressible

This package owns one reusable steady incompressible Newtonian volume law and
one separate complete-exterior mechanical interface. The volume Component
binds root-owned velocity, pressure, and conservative force-potential Fields.
The boundary Component binds velocity trace and parent-outward Cauchy traction
to exact `Eqiora.Mechanics.Interfaces::VelocityTractionBoundary` Ports.

The Components expand into ordinary typed Relations. An enclosing Model supplies
the Domains, Fields, boundary data, and terminal connections, then selects its
mesh and numerical policies independently. The velocity/traction Connector
remains distinct from the quasistatic displacement/traction Connector.
