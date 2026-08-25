# Installed Python authored-CAD graph verification

This case verifies a transparent installed-Python projection of the existing
native authored-CAD graph. `eqiora.geometry.CadAuthoredGraph` constructs the
accepted rectangle extrusion, returns an immutable successor for its one
circular through-cut, replays either frozen canonical wire, exposes graph-bound
face handles, and returns the complete native analytic build receipt.

For the accepted circular-through-cut history, Python obtains one immutable
`result = graph.planar_result()`. The result executes and retains the accepted
build once. `result.project(handle)` accepts retained handles captured from the
exact pre-Boolean graph and the created cut-wall handle from the final graph;
foreign revisions, lookalike predecessors, wrong handle generations, deletion,
and ambiguity reject. The sole `result.with_named_topology(mapping)` call then
binds arbitrary names to opaque result handles and publishes Geometry v2 only
after complete exactly-once coverage. No caller classification tolerance,
coordinates, proximity, provider labels, or mesh labels participate. The older
circle-shaped Geometry v1 method remains temporarily for compatibility.

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
planar identity. The result mapping route performs no geometric classification.
Python Geometry owns v1 or v2 directly; v2 reports
`classification_tolerance is None`. Source-owned v2 Mesh correspondence is not
part of this case, so v2 mesh operations reject explicitly rather than
manufacturing v1 content.

The registered executor rebuilds and installs a non-editable wheel, runs the
complete public Python contract, and launches a second isolated interpreter.
Run:

```bash
cargo test -p eqiora-python --test python_cad_authored_graph
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-cad-authored-graph
```

This case does not claim generic CAD operations, arbitrary profiles, Booleans
or sections, general primitive/subtract result ergonomics, a public feature
enum, persisted result-handle schema, v2 meshing, Model construction, solve
semantics, Studio section projection, performance, or physical validation.
Authored-graph identity and derived output-Geometry identity remain distinct.
