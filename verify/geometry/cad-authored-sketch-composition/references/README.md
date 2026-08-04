# References

This composition oracle consumes exactly three accepted authorities:

- [`geometry.cad-authored-rectangle-extrusion`](../../cad-authored-rectangle-extrusion/README.md)
  owns the exact v1 graph, canonical face order, observations, durable v1
  handles, and decode/re-encode behavior.
- [`geometry.cad-authored-circular-through-cut`](../../cad-authored-circular-through-cut/README.md)
  owns the exact v2 graph, signed-clearance predicate, topology observations,
  requested/effective tolerance receipt, lineage membership/counts, and
  canonical v2 handles. Its accepted executor also derives the planar
  authority from a distinct DFG-sized graph, not from the symmetric v2 graph.
- [`geometry.exact-circular-hole-geometry`](../../exact-circular-hole-geometry/README.md)
  owns the exact DFG-shaped transverse section bytes and digest. This case
  reaches it only through the separate `[0,2.2] × [0,0.41]` graph with center
  `[0.2,0.2]` and radius `0.05`.

This case freezes only native composition over these authorities. It adds no
provider reference, scientific derivation, expected value, tolerance, wire,
digest family, or alternate presentation authority.
