# Independent accepted-step oracle

`accepted-step.json` is evidence-only data consumed by the Rust integration
test. It is not an Eqiora artifact, a serialized accepted result, a stable
schema, or a publication contract.

The fixture freezes the exact vertex and tetrahedron order before recording
topology-dependent nodal reactions. Its values come from the issue's two
independent derivations: an affine continuum patch and exact-rational P1
tetrahedron assembly. In particular, the positive patch has zero acceleration,
so the separate density-inclusive center mass block is mandatory evidence.

The reaction sign is constraint-on-body. Consequently the fixed `x=0` face
totals `-0.09 N` and the driven `x=1` face totals `+0.09 N`. Unequal corner
weights are intentional and reject equal four-corner splitting or a changed
face diagonal paired with stale node-indexed reactions.

All numbers and tolerances are precommitted. They must not be changed to match
an implementation. The case makes no general validation claim for nonzero
acceleration, nonzero first Lame parameter, another material, or another mesh.
