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

The final native replay statement is compositional rather than a second
subprocess transport: the installed test returns the exact accepted 731- and
1292-byte canonical wires, the renderer tests replay requests made from those
same owner bytes, and `interfaces.studio-cad-authored-graph` already compares
the resulting native projection with the independently constructed owner field
for field. No digest-only equality is used to bridge those checks.

Scalar spelling is also frozen. It uses Rust's shortest round-trip debug form
for finite canonical `f64` values, while retaining `.0` on integer-valued
floats. Exponents have no `+` sign or zero padding (`1e-9`, not `1e-09`). This
is an Eqiora source convention expressed in valid Python syntax, not Python's
`repr` convention. The fixtures are formatter-canonical and must remain so.

[`models/hostile.json`](models/hostile.json) precommits the request, response,
staleness, cancellation, and write-error mutants before production code exists.
The implementation may wire those cases to its closed DTOs but may neither
remove a mutant nor weaken its expected disposition.

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
