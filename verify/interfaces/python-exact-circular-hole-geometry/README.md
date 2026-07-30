# Python exact circular-hole geometry verification

This case freezes one deliberately bounded Python authoring surface:
`eqiora.geometry.RectangleWithCircularHole`. The native value is owned by Rust
and delegates exact geometry meaning to the existing canonical
axis-aligned-rectangle-with-one-circular-hole family. It exposes immutable
bounds, circle data, tolerance, canonical bytes, digest, and the names and
dimensions of its fixed-role selections. The constructor assigns six semantic
roles: one region plus x-lower, x-upper, y-lower, y-upper, and hole
boundaries. Equal same-dimensional names group roles into one selection, so
the standard witness exposes five names because both y roles are `walls`.

The repository-owned standard byte oracle remains
[`examples/steady-flow-past-cylinder.geometry.json`](../../../examples/steady-flow-past-cylinder.geometry.json).
The test removes exactly its terminal newline and requires byte equality; this
case does not independently reinterpret that geometry formula.
[`geometry.exact-circular-hole-geometry`](../../geometry/exact-circular-hole-geometry/README.md)
remains authoritative for the wire encoding, framed digest, geometry
predicates, and semantic admission. The Rust-side Python test also decodes the
published Python bytes through that public Rust contract and compares the
complete digest.

The positive witness groups both y sides under the one `walls` selection.
Signed zero has one equality, hash, byte, and digest identity. Swapping inlet
and outlet role names changes identity. Falsifiers require structured
`ValidationError` for invalid exact geometry, ambiguous cross-dimensional
selection naming, and an unknown selection lookup. Unexpected mesh sizing,
circle segmentation, or approximation arguments are not accepted.

Because the grouped `walls` witness cannot distinguish the two y sides, an
independent oriented witness names y-lower `floor` and y-upper `ceiling`. Its
RFC-derived 556-byte canonical content and digest
`51ece8fa2d8709d932b0c758d59c187e4fd572f73217c31dcbe407f8d873be7f`
pin `floor` to canonical boundary member 2 and `ceiling` to member 3. Reversing
the implementation mapping therefore fails exact content and identity, rather
than merely producing a second unequal value.

The standard centre has equal coordinates and therefore cannot expose an x/y
transpose. A second valid wrapper witness uses centre `(0.3, 0.2)`, while
retaining the exact same bounds, radius, tolerance, entity sets, schema, and
canonical-number rules. Its public getter must return `(0.3, 0.2)`, its exact
511-byte content is pinned, and its digest is
`552ebf459396ed5bc7f72ab48f34046baa828b6af808794e861bd958dc613881`.
Transposing either constructor pass-through or getter order therefore fails
independently of the symmetric standard witness.

The registered executable case is the package gate: it rebuilds and installs
the non-editable wheel before running the public Python contract. That
contract also launches the checked-in
`examples/python/exact_cylinder_geometry.py` with the installed-wheel
interpreter and pins its digest and five selection/dimension lines. The
in-process Rust test remains supplemental cross-language coverage. Run:

```bash
cargo test -p eqiora-python --test python_geometry_authoring
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-exact-circular-hole-geometry
```

This case does not claim generic rectangles, circles, Boolean construction,
general selection queries, geometry or boundary handles, mesh generation,
import, Model construction, solve, Result, visualization, performance, or
physical validation.
