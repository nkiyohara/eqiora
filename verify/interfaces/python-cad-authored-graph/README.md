# Installed Python authored-CAD graph verification

This case verifies a transparent installed-Python projection of the existing
native authored-CAD graph. `eqiora.geometry.CadAuthoredGraph` constructs the
accepted rectangle extrusion, returns an immutable successor for its one
circular through-cut, replays either frozen canonical wire, exposes graph-bound
face handles, and returns the complete native analytic build receipt.

For the accepted circular-through-cut history, Python can also request the
exact transverse planar section with an explicit classification tolerance and
semantic role names. Native Rust derives it through the existing exact planar
owner. The standard DFG route reproduces its previously frozen 511 canonical
bytes and digest exactly; the ordinary installed-package cylinder examples now
author the graph before following the unchanged mesh and solve path.

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

A rectangle-only graph cannot produce the circular section. A
cut-admitted narrow-clearance graph still rejects when the separately supplied
Geometry classification tolerance is too large, while plane z, extrusion
depth, and modeling tolerance cannot leak into transverse planar identity.

The registered executor rebuilds and installs a non-editable wheel, runs the
complete public Python contract, and launches a second isolated interpreter.
Run:

```bash
cargo test -p eqiora-python --test python_cad_authored_graph
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-cad-authored-graph
```

This case does not claim generic CAD operations, arbitrary profiles, Booleans
or sections, a public feature enum, meshing, Model construction, new solve or
Result semantics, Studio section projection, performance, or physical
validation. Authored-graph identity and derived output-Geometry identity remain
distinct.
