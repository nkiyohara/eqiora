# References

This installed-Python composition oracle consumes exactly four accepted
authorities:

- [`geometry.cad-authored-sketch-composition`](../../../geometry/cad-authored-sketch-composition/README.md)
  owns the opaque native sketch contract, explicit composition meaning,
  admission boundary, operation order, lifetime, and equality semantics.
- [`geometry.cad-authored-rectangle-extrusion`](../../../geometry/cad-authored-rectangle-extrusion/README.md)
  owns the exact 731-byte v1 graph, digest, canonical face order,
  observations, handles, and decode/re-encode behavior.
- [`geometry.cad-authored-circular-through-cut`](../../../geometry/cad-authored-circular-through-cut/README.md)
  owns the exact 1292-byte v2 graph, strict signed-clearance predicate,
  observations, requested/effective tolerance receipt, lineage
  membership/counts, and canonical handles.
- [`interfaces.python-cad-authored-graph`](../../python-cad-authored-graph/README.md)
  owns the installed-Python graph, handle, observation, receipt, lineage, and
  exact planar-section projection of those native authorities. The separate
  [`interfaces.python-exact-circular-hole-geometry`](../../python-exact-circular-hole-geometry/README.md)
  case owns the DFG-shaped 511-byte Geometry and unchanged example output.

This case adds no alternate scientific derivation, expected value, tolerance,
wire, digest family, provider reference, Model, or presentation authority.
