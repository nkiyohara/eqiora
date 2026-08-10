# Exact collocated `4 x 6 x 8` Cartesian periodic view

This case verifies one crate-private structural view for the exact accepted
current Model and the one route-sealed uniform Cartesian mesh whose cell counts
are `4 x 6 x 8`. The ordinary path compiles the existing RFC 0071 source,
round-trips its Transaction and Model artifacts, replays whole-Model validation,
builds the mesh from the contract's IEEE-754 hex literals, round-trips the mesh
artifact, and calls the real private collocated projection.

The independent oracle then checks the Model, mesh, parent, Connector, ordered
Connection triple, private inventory receipt, event order, and every field of
all 576 returned packets. Expected neighbours and quotient faces come from the
accepted last-axis-fastest and modulo laws, not from a production derivation or
admission helper. The ordinary positive completes before the six input
falsifiers, the admission-order control, and the thirteen observation mutants.

Run the registered case after the integration owner adds its test-only module
registration:

```bash
mise run affected -- --case fluid.cartesian-periodic-collocated-view-3d
```

The claim is deliberately narrower than a Taylor--Green demo. It does not
verify a numerical operator, residual or JVP, pressure, gauge, conservation,
energy, solver, time integration, campaign, trajectory, gallery, media, or
publication result. It also does not claim another Model, mesh, count tuple, or
coordinate evaluation route; a literal collocated-mesh digest; arbitrary-count
or dimension-parametric behavior; mesh-faithful axis-1 seam distance; lifted
seam point sets; exterior-face absence; stored-scatter falsification; a public
or persisted topology surface; MPI/GPU; portability; performance; scale;
memory; or wall-clock behavior.
