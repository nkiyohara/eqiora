# Atomic Parameter-driven Cartesian regeneration

This case proves one application-owned regeneration operation over the direct
coordinate recipe accepted by
`geometry.direct-parameter-cartesian-coordinates`. One current-v8 root length
Parameter drives the x-upper and y-lower endpoints of one three-dimensional
Cartesian body. Preview changes that Parameter from `2 m` to `3.5 m`, resolves
the complete before/after geometry, and returns an immutable plan only after
the exact child Model has replayed.

The transaction is the ordinary value path: `RevisionIs`, `ValueEquals`, and
exactly one Parameter `SetValue`. It contains no Domain removal, definition,
or edge reconnection. The Domain recipe, every node and edge, body and boundary
identities, and all six Cartesian roles remain exact.

Two isolated Opus 5 sessions derived the scientific oracle without reading the
implementation or existing fixtures. One used exact rational arithmetic; the
other used independent width products and a coupled checksum. Both obtain
target bounds `[-1, 3.5]`, `[3.5, 6]`, `[0.5, 5.5]` and exact volume
`56.25 m^3`. The x/y width sum remains `7 m`, while their difference changes
from `-1 m` to `2 m`. An x-only mutant instead has sum `8.5 m` and volume
`90 m^3`, so partial propagation cannot pass.

The independently compiled target is a metric oracle, not an assertion of
whole-Model structural equivalence: the regenerated child retains the original
Parameter definition and changes its revision-local value, while the target
source declares `3.5 m` initially. Source rewriting is outside this case.

The child changes both Model and Geometry Identity digests. Selection retention
is accepted only through the existing explicit total one-to-one geometry
revision association, whose replay retains the body and every boundary Domain.
Reversing every declaration-order array in the exact Model wire before replay
produces equal plans, transaction bytes, child bytes, and digests. Positive and
negative zero requests also canonicalize identically.

Run:

```bash
cargo test --locked -p eqiora --test parameter_geometry_regeneration
cargo run --locked -p eqiora-verify -- run --case geometry.parameter-cartesian-regeneration
```

This case does not claim multiple Parameters or Domains, source rewriting,
Component forwarding, generic expressions, Python, Studio, CAD-kernel or mesh
regeneration, ALE, optimization, or shape sensitivity.
