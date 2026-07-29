# Studio unstructured P1 scalar Field

This case registers one closed application projection for a coherent-SI scalar
P1 Field on a bounded two-dimensional affine-triangle mesh. The ordinary
positive fixture is the existing fixed-reference FSI pressure snapshot, not a
cylinder-specific shortcut. `eqiora-api` replays the exact Model, semantic
revision, Realization plan, geometry/correspondence/mesh lineage, Field,
Domain, logical snapshot, and vertex coefficient block before it copies any
renderer-ready data. The logical snapshot digest must also be a registered
output of the exact Run; matching Model and Realization lineage alone is not
enough.

Studio retains at most two accepted projections. Its descriptor carries the
same artifact identities and declares three separate fixed-order streams:
coordinates as `f64-le`, triangle connectivity as `u32-le`, and vertex values
as `f64-le`. Every payload begins with the closed v1 16-byte little-endian
header carrying its magic, stream, chunk index, and item count, so a duplicated
or reordered equal-length payload cannot borrow the request's identity. The
session requests each chunk once in canonical order and does not publish a
ready state until headers, byte shapes, finite coordinates/values, mesh bounds,
positive connectivity, final counts, and exact extrema all agree. Changing
context invalidates outstanding asynchronous work.

The canvas is presentation only. Its backing resolution and triangle-pixel
work are bounded independently of the accepted data-plane sizes; exceeding
that presentation budget leaves the exact synchronized paged table, inspector,
and lineage available. Keyboard selection always names a canonical mesh vertex
rather than an interpolated probe.

Run:

```bash
cargo test --locked -p eqiora --test unstructured_p1_scalar_studio_projection
npm --prefix studio run check
npm --prefix studio test
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-unstructured-p1-field
```

This does not claim vectors, derived magnitude, contours, streamlines, glyphs,
probes, moving-mesh deformation, animation, 3D, export, a new durable
visualization/transport artifact, GPU zero-copy, level of detail, or
production-scale visualization.
