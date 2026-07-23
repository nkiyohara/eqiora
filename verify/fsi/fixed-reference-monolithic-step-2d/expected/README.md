# Expected evidence

The registered test accepts only when all of the following close together:

- direct and exact-package Models identify the same fluid, solid, and
  interface roles and finalize the same dimensionless CSR/RHS;
- exact Model, geometry, correspondence, mesh, Realization, and Run references
  replay without drift;
- the shared interface velocity is bit-identical and nonzero;
- fluid and solid body-cut weak actions sum to zero on every free interface
  row;
- weak incompressibility and `d_next - d_previous - dt * v_next` close;
- independent CSR reapplication reproduces the finalized right-hand side;
- the backward-Euler kinetic, elastic, viscous, and numerical-increment energy
  terms close with zero external work; and
- repeated finalization produces the same canonical operator fingerprint.

Mutation checks reject stale content, partial or wrongly oriented interfaces,
wrong typed physical boundaries, unrepresented live Ports, incorrect mass or
kinematic blocks, a broken quotient, a one-sided traction sign, stale pressure
gauging, and cross-wired time/state/execution inputs.
