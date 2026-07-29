# Authored planar geometry artifact verification

This case proves one externally supplied, straight-edged planar geometry
artifact. The positive fixture is a unit square with a centred square hole,
using only dyadic coordinates and tolerance so its independent decimal
encoding is exact.

The Rust evidence reconstructs the same validated `PlanarRegion`, compares its
complete canonical bytes and domain-separated digest with the frozen oracle,
then admits those external bytes through the bounded decoder and replays the
same region. Mutants change whitespace, author order, loop rotation, topology,
entity membership, entity-set name uniqueness, wire vocabulary, and every
decoder budget. One exact-boundary mutant proves that 4,096 loop indices reach
topology validation, while a 4,097-index mutant proves that the shipped ceiling
binds first. Separate regressions cover multiple-hole ordering, duplicate-hole
rejection, member deduplication, and the repository serializer's exact
non-dyadic binary64 spelling. A separate regression retains an unreferenced
vertex, proves that it changes canonical loop indices and artifact identity,
and externally re-admits the resulting bytes.

The Python program under `expected/` independently derives the compact JSON and
SHA-256 from RFC 0079. It neither reads Rust source nor consumes Rust output.
The Rust test executes that derivation, and both routes must agree on 482 bytes
and digest
`e6f8e17ac215ef37ca3c9de07b9979e34f13412a5de11dc9240ea1def8130030`.
The registered Rust evidence therefore requires a host `python3` interpreter
and fails rather than skipping if it is absent.

The sweep-line differential test uses an independent all-pairs traversal but
shares the frozen `hypot` metric; fixed overflow, underflow, signed-zero, and
exact-tolerance cases separately falsify that metric.

Run:

```bash
python3 verify/geometry/authored-planar-geometry-artifact/expected/derive_digest.py
cargo test -p eqiora-artifact --test geometry_definition_wire
cargo run -p eqiora-verify -- run --case geometry.authored-planar-geometry-artifact
```

This case does not claim Model admission, semantic spatial support, source
syntax, mesh correspondence or realization, curves, 3D geometry, booleans,
CAD, filesystem loading, robust exact-real intersection or containment
predicates, pruning of unreferenced vertices, or geometric equivalence under
tolerance or motion.
