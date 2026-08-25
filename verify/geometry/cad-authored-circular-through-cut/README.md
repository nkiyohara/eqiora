# Authored circular through-cut

This case proves one topology-changing operation on the common immutable CAD
authoring owner. A fully constrained rectangular face is extruded in positive
z; a strictly interior circle on its end cap is then cut through all in
negative z. The result remains exact analytic meaning: the circle and cylinder
are never replaced by chords, elements, or renderer facets.

The final graph now admits one immutable planar result after executing its
analytic build once. The build receipt owns the complete exact relation from
pre-Boolean construction handles to result dimension/member identity. Retained
side and end-cap handles must belong to the exact predecessor v1 graph; the
created cut-wall handle must belong to the final v2 graph. Wrong generations,
lookalike predecessors, deleted topology, and ambiguity reject rather than
falling back to coordinates or proximity.

After projection, one atomic name-to-result-handle mapping must cover the
complete planar topology exactly once. It produces the accepted
provenance-neutral planar circular-hole Geometry v2 without classification or
tolerance input. The older circle-shaped Geometry v1 method remains temporarily
as a compatibility route. The v2 exact bytes and digest remain owned unchanged
by [`geometry.planar-circular-hole-geometry-v2`](../planar-circular-hole-geometry-v2/README.md).

Opus 5 and Fable 5 independently derived the geometry and topology before
implementation. Both routes agreed on the 1292-byte v2 wire, digest
`00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47`,
seven face observations, body volume, surface area, and complete lineage. The
accepted binary64 gate is pure relative `4e-15`, derived from the permitted
seven-term sum error and separated from every precommitted algebra mutant by
more than eleven orders of magnitude.

The build receipt keeps the identity-only base modeling tolerance separate
from the Boolean request and effective policy. A second witness with base
`1e-10 m` and requested/effective Boolean tolerance `1e-11 m` rejects base,
minimum, or maximum clamping. The analytic profile is named explicitly and
reports zero by-construction positional, area, and volume discrepancies plus
`repair = none`; it is not presented as Truck or another numerical kernel.

Relative to the six-face predecessor, four lateral faces are retained
unchanged, both caps retain provenance while gaining an inner boundary cycle,
and one cylindrical cut wall is created. Deleted, split, and merged inventories
are explicitly empty. Edge and vertex counts remain nonclaims because CAD
providers may represent cylindrical seams differently.

Run:

```bash
python3 verify/geometry/cad-authored-circular-through-cut/oracle.py
cargo test -p eqiora-geometry --test cad_authored_circular_through_cut
cargo run -p eqiora-verify -- run --case geometry.cad-authored-circular-through-cut
```

This is not general primitive/subtract result ergonomics, a feature DAG, B-rep
or CSG schema, arbitrary section extractor, multiple/blind-hole system,
production CAD-kernel Boolean, healing or per-entity tolerance system, mesh,
solver, Studio projection, or renderer claim.
