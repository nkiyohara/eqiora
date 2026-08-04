# Bounded authored CAD sketch composition

This preimplementation case freezes one opaque native Rust sketch owner over
the two closed inputs already admitted by the authored CAD graph. A constrained
XY rectangle plus its requested modeling tolerance may produce only the
accepted positive-z extrusion. An exact circle bound to the predecessor v1
graph's canonical `end-cap` handle may produce only the accepted circular
through-cut. No general Sketch, constraint, plane, profile, feature, Boolean,
or operation graph is claimed.

For the explicit rectangle route, the oracle replays the accepted 731-byte v1
graph and digest
`919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36`.
For the explicit cut route, it replays the accepted 1292-byte v2 graph and
digest
`00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47`.
That symmetric graph owns only the accepted v2 bytes, handles, observations,
receipt, and lineage; this case makes no assertion that its planar projection
is the unrelated DFG witness.

A separate graph uses rectangle bounds `[0.0,2.2] × [0.0,0.41]`, plane `0`,
modeling tolerance `1e-10`, depth `1.0`, circle center `[0.2,0.2]`, radius
`0.05`, and Boolean tolerance `1e-10`. Its explicit and compatibility routes
must compare exactly before independently supplied planar classification
tolerance `1e-12` and the frozen entity roles derive the accepted 511-byte
exact planar geometry with digest
`b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`.
The explicit and compatibility routes must agree byte-for-byte and
digest-for-digest, including face handles and order, all public graph and face
observations, complete build receipt and topology lineage, and canonical
decode/re-encode behavior.

The lineage assertions compare each explicit-route vector with its
compatibility-route vector and separately check the inherited category
membership, counts, and complete face-inventory partition. They intentionally
do not freeze a new internal vector order, complete handle-envelope literal,
created-lineage literal, or provider-profile literal. The predecessor cases
remain authoritative for those accepted contracts.

The rejection corpus covers rectangle-coordinate ownership, zero, negative,
nonfinite, and signed-zero-sensitive tolerances, depths, centers, and radii;
foreign, stale, differently tolerant, wrong-face, and v2 handles; exact and
asymmetric signed-clearance boundaries; wrong operation order; and clone,
move, and inline argument construction. NaN and both infinities are exercised
independently in each center coordinate. Every rejection must retain the
existing `EQ0901` invalid-artifact class.

Signed-zero ownership is deliberately split. Rectangles with positive-zero
and negative-zero lower bounds must produce equal v1 graphs, and no cut is
applied to those boundary-zero rectangles. Separately, the accepted symmetric
v1 authority is cut with centers `[0.02, +0.0]` and `[0.02, -0.0]`, radius
`0.008`, and Boolean tolerance `1e-9`; both routes must succeed, compare equal,
and reproduce the exact accepted v2 authority. The DFG planar proof is owned by
the separate graph fixture above. Equality at the strict-clearance boundary and
the asymmetric near-boundary mutant still reject.

Run after the implementation and integration lanes provide the frozen public
API:

```console
cargo test -p eqiora-geometry --test cad_authored_sketch_composition
cargo run -p eqiora-verify -- run \
  --case geometry.cad-authored-sketch-composition
```

At the frozen preimplementation revision, `CadAuthoredSketch` and
`CadAuthoredGraph::through_cut` intentionally do not exist. This target must
therefore fail only at that missing future public API boundary. The standalone
predecessor Python authorities and every existing predecessor Rust target must
already pass.

This case does not claim Python or Studio projection, a wire or digest change,
arbitrary face-local coordinates, import/export, mesh generation, multi-body
behavior, more than one cut, mutation/history editing, or any new numerical or
scientific tolerance.
