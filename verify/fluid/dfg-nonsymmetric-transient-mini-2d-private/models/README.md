# Model construction

There is no durable `.eqi` fixture here. The Rust oracle derives the exact DFG
source from the already accepted non-box transient test scaffold before
compilation. The derived source changes only the frozen NUM0 meaning:

- density `rho=1`, dynamic viscosity `mu=0.001`, and zero force potential;
- `sigma_DFG = mu grad(velocity) - isotropic_lift(pressure)` in both volume
  and outlet;
- exact paired `div(velocity)=0`;
- `inlet_profile = 4 Umax y(H-y)/H^2` with `Umax=0.3`, `H=0.41`;
- `trace(velocity)+normal(isotropic_lift(inlet_profile))=0` on `inlet`;
- zero trace on `walls` and `cylinder`; and
- zero DFG natural traction on `outlet`.

The Cartesian authoring is only a compiler scaffold. The test replaces it
with the accepted exact `GeometryRegion`/named `GeometryBoundary` owner and
requires the public Cartesian lowerer to remain symmetric-only. No runtime
stress option, model file, public type, durable wire, or second semantic
registry is introduced.

The initial state is not selected from candidate DFG output. The evidence
constructs the correspondence-owned essential lookup, then uses the already
accepted steady MINI/P1 solver with the same mesh, inlet trace, zero walls and
cylinder, and traction-pressure closure. This independently produces a
finite, nonzero, weakly continuous shaped state before the DFG advance exists.
