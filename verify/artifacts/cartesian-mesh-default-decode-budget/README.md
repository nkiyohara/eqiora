# Cartesian mesh default decode budget

This case freezes the resource policy for decoding an axis-compressed
`CartesianMeshEnvelopeV1`. It does not change the Cartesian wire. The ordinary
untrusted path remains `from_json(bytes, MeshDecoderLimits)` and delegates to
the Cartesian default policy; an explicit caller uses
`from_json_with_limits(bytes, CartesianMeshDecoderLimits)`.

For one axis with `n_i` stored vertices, a Cartesian entity either fixes that
axis at one of `n_i` vertices or is free over one of `n_i - 1` intervals.
Multiplying those independent axis states gives the total all-strata entity
inventory:

```text
E = product(2 * n_i - 1)
```

A fixed axis contributes one closure vertex and a free axis contributes two.
Weighting the same states by their closure vertices gives the total number of
entity-closure vertex references:

```text
R = product(3 * n_i - 2)
```

The oracle commits literal values for every witness. The Rust test generates
only the axis coordinate arrays; it deliberately has no helper that computes
expected `E` or `R` with the production formula.

Default decoding admits `E <= 1_000_000` and `R <= 8_000_000`, inclusively.
Exact-limit, one-below, nonuniform, entity-only, closure-reference-only, and
checked-overflow witnesses distinguish the intended calculation from vertex,
top-cell, or top-connectivity substitutes. The 12-axis two-point witness must
be rejected by the Cartesian decoder's closure-reference diagnostic before
the lower `ReferenceTopology` path can own the error.

Trusted capture is intentionally asymmetric. A validated 501-by-501
`CartesianMesh` has literal `E = 1_002_001` and `R = 2_253_001`.
`from_mesh` captures it under the meshing hard ceiling, untrusted default decode
rejects its canonical bytes, and an explicit exact admitting policy replays
the same bytes, digest, and mesh ordering. Trust is a caller-selected resource
context; it is not serialized or inferred from provenance.

The accepted 17-by-17 envelope remains an exact compatibility witness. Its
462 canonical bytes and domain-separated SHA-256 digest are committed in the
test, so adding policy data to the wire or changing existing ordering fails.

This case does not measure heap bytes, allocator layout, wall-clock time, or a
portable performance bound. It does not claim a new mesh family, imported or
nonuniform product workflow, universal policy registry, scientific result, or
relaxation of the meshing layer's hard in-memory caps.

Run:

```bash
cargo test -p eqiora-artifact --test cartesian_mesh_decode_budget
cargo run -p eqiora-verify -- run \
  --case artifacts.cartesian-mesh-default-decode-budget
```
