# Installed Python authored-CAD graph verification

This case verifies a transparent installed-Python projection of the existing
native authored-CAD graph. `eqiora.geometry.CadAuthoredGraph` constructs the
accepted rectangle extrusion, returns an immutable successor for its one
circular through-cut, replays either frozen canonical wire, exposes graph-bound
face handles, and returns the complete native analytic build receipt.

For the accepted circular-through-cut history, Python can also request the
exact transverse planar section by passing one `named_topology` mapping to
`graph.planar_section(...)`. Values are graph-bound face handles or
same-dimensional sequences of them; arbitrary user names are admitted only
when the complete section topology is named exactly once. Native Rust derives
the roles from build lineage and rejects foreign/stale handles, omissions,
duplicates, mixed dimensions, coordinates, provider labels, and malformed
mapping values. No caller classification tolerance is involved. The standard
DFG mapping reproduces its previously frozen 511 canonical bytes and digest
exactly. The predecessor circle-shaped method remains temporarily available to
the existing installed-package examples until their separate migration.

The adapter owns no CAD meaning and introduces no numeric oracle. The v1
731-byte wire and digest come from
[`geometry.cad-authored-rectangle-extrusion`](../../geometry/cad-authored-rectangle-extrusion/README.md);
the v2 1292-byte wire, exact observations, tolerance policy, and lineage come
from
[`geometry.cad-authored-circular-through-cut`](../../geometry/cad-authored-circular-through-cut/README.md).
The Python test embeds those already accepted values and a supplemental Rust
test replays Python-produced bytes through the public native decoder.

`graph_digest` deliberately names authored-history identity. A
modeling-tolerance-only witness has a different graph digest while retaining
equal geometry observations, and neither graph nor build exposes a misleading
`geometry_digest`. Canonical wires and foreign graph-bound handles fail closed
in the native owner rather than being interpreted in Python.

A rectangle-only graph cannot produce the circular section. The predecessor
compatibility route still rejects a cut-admitted narrow-clearance graph when
its separately supplied Geometry classification tolerance is too large, while
plane z, extrusion depth, and modeling tolerance cannot leak into transverse
planar identity. The atomic mapping route performs no geometric
classification.

The registered executor rebuilds and installs a non-editable wheel, runs the
complete public Python contract, and launches a second isolated interpreter.
Run:

```bash
cargo test -p eqiora-python --test python_cad_authored_graph
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-cad-authored-graph
```

This case does not claim generic CAD operations, arbitrary profiles, Booleans
or sections, a public feature enum, persisted topology-name schema, meshing,
Model construction, new solve or Result semantics, Studio section projection,
performance, or physical validation. Authored-graph identity and derived
output-Geometry identity remain distinct.
