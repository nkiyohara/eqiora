# Reference provenance

This is an independent pre-implementation evidence package. It composes
already accepted authorities without using values emitted by the new Python
adapter:

- `fluid.exact-circular-hole-stokes-2d-gmsh` owns the dual-independent pressure,
  flux, force, balance, solver, and residual observations;
- `artifacts.current-model-canonical-identity` owns semantic Model identity;
- `interfaces.python-exact-circular-hole-geometry` owns the exact source;
- `interfaces.python-circular-hole-chordal-mesh` owns the source-bound inner
  mesh; and

The Python tests independently derive the model digest and replay the binding
and Run v2 digests from exposed canonical bytes. Produced digests, a complete
pressure hash, exact extrema, and exact residual values are intentionally not
copied into the oracle.
