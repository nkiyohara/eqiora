# Studio authored-CAD to Python round trip

This case freezes the exact readable Python source for the two already accepted
authored-CAD histories: one rectangle extrusion and its optional one circular
through-cut successor. The programs call only the installed public
`eqiora.geometry.CadAuthoredGraph` surface and leave the reconstructed graph in
the stable top-level name `authored_graph`.

The installed-wheel test copies each frozen program outside the repository,
executes it from an empty working directory with an isolated interpreter, and
compares the result with the corresponding public constructor path. Equality
includes canonical graph bytes and digest, ordered graph-bound face handles,
all public exact observations, and the complete analytic build receipt. The
native Studio renderer must separately equal these source files byte for byte.

The graph and scientific authorities remain
[`geometry.cad-authored-rectangle-extrusion`](../../geometry/cad-authored-rectangle-extrusion/README.md)
and
[`geometry.cad-authored-circular-through-cut`](../../geometry/cad-authored-circular-through-cut/README.md).
This adapter case does not introduce or tune a geometric tolerance or expected
numeric value.

Run:

```bash
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run --case interfaces.studio-python-cad-round-trip
```

This case does not claim arbitrary Python code generation, Python-to-Studio
parsing, arbitrary CAD histories, pure output-Geometry identity, mesh, Model,
physics, solve, visualization, or cross-version generated-source compatibility.
