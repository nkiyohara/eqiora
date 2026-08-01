# Installed Python authored-CAD graph verification

This case verifies a transparent installed-Python projection of the existing
native authored-CAD graph. `eqiora.geometry.CadAuthoredGraph` constructs the
accepted rectangle extrusion, returns an immutable successor for its one
circular through-cut, replays either frozen canonical wire, exposes graph-bound
face handles, and returns the complete native analytic build receipt.

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

The registered executor rebuilds and installs a non-editable wheel, runs the
complete public Python contract, and launches a second isolated interpreter.
Run:

```bash
cargo test -p eqiora-python --test python_cad_authored_graph
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-cad-authored-graph
```

This case does not claim generic CAD operations, arbitrary profiles or
Booleans, a public feature enum, output-Geometry identity, meshing, Model
construction, solve, visualization, Studio integration, performance, or
physical validation.
