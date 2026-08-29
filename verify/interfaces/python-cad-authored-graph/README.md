# Installed Python Geometry solid-operation verification

This case verifies the solid-operation family under the single installed-Python
`eqiora.geometry.GeometryGraph` owner. The graph constructs the accepted
rectangle extrusion, creates its one circular-through-cut successor, binds
exact revision-owned face handles, decodes either accepted canonical wire, and
publishes either a complete analytic `GeometryBuildReceipt` or the admitted
common planar `Geometry`.

The v1 731-byte wire and digest come from
[`geometry.cad-authored-rectangle-extrusion`](../../geometry/cad-authored-rectangle-extrusion/README.md);
the v2 1292-byte wire, observations, tolerance policy, and lineage come from
[`geometry.cad-authored-circular-through-cut`](../../geometry/cad-authored-circular-through-cut/README.md).
The installed-wheel test compares both exact wires and digests without deriving
new numerical values.

`graph_digest` remains authored-operation identity and is distinct from output
`Geometry` identity. A decoded operation is rebound to the receiving
`GeometryGraph`; foreign operations and handles, predecessor-revision handles,
and incomplete naming reject before a successor or Geometry can be published.
The displaced `CadAuthoredGraph`, `CadAuthoredSketch`, `CadAuthoredBuild`, and
`CadAuthoredFaceHandle` public generation is absent rather than aliased.

Run:

```bash
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.python-cad-authored-graph
```

This case does not claim generic CAD operations, arbitrary profiles or
Booleans, a universal operation enum, general 3D `Geometry`, meshing, Model,
solve, Result, performance, or physical validation.
