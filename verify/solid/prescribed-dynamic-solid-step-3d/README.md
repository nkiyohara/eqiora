# Prescribed dynamic-solid step in 3D

This case admits one serial-host in-memory backward-Euler step for the exact
unit-cube P1 tetrahedron fixture in `models/direct.eqi`. The `x=0` face has
zero velocity, `x=1` remains a live velocity/traction `PortBinding`, and the
other four faces have zero traction. The L3 candidate supplies total next
displacement at the driven vertices; it does not introduce a displacement
boundary law into canonical Model meaning.

The Rust test constructs the fixed Model, geometry identity, mesh envelope,
and correspondence, then binds complete identity-tagged prior displacement and
velocity fields. It checks projection, generation-bound candidate admission,
the affine accepted fields, separate mass and stiffness operators, the reduced
backward-Euler system, constraint-on-body reactions, residuals, and existing
assembly/solve evidence.

Falsifiers cover stale generation and lineage; wrong field or candidate shape,
order, identity, duplication, and finiteness; invalid time quantities; a
changed boundary triangulation; total-versus-increment interpretation; stale
boundary velocity; the listed mass, stiffness, and reaction mutants; and
atomic state under validation, assembly, and solver failures.

This is not a durable Realization, State, Run, trajectory, provider protocol,
Python, Studio, parallel, arbitrary-mesh, or general dynamic-solid claim.
