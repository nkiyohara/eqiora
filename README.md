# Eqiora

Eqiora is an open-source computational engineering platform that represents
models as one typed network of mathematical relations, then carries that
meaning through numerical realization to auditable evidence.

> **Alpha — `0.1.0a7`.** Eqiora is research software under active development.
> The [capability matrix](docs/capability-matrix.md) shows what is available in
> the current release.

## Start with Python

The alpha distribution supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64:

```console
python -m pip install "eqiora[gmsh]==0.1.0a7"
```

Build an exact rectangle-with-circular-hole geometry, resolve its Gmsh mesh,
run the accepted steady-Stokes application, and inspect typed outputs and
boundary observables from the immutable `Result`:

![Fine-mesh exact-cylinder steady-Stokes pressure field](docs/site/src/assets/gallery/exact-cylinder-pressure-presentation.png)

```python
from importlib.resources import files

import eqiora

graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:4],
    "cylinder": circle.boundaries[0],
})
mesh_request = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    maximum_target_size=0.025,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
mesh = eqiora.meshing.generate(mesh_plan)

model = eqiora.compile(
    path=files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi"),
    geometry=geometry,
    parameters={
        "dynamic_viscosity": 1.0e-3,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
    },
)
linear = eqiora.solve.Linear(
    relative_tolerance=1e-6,
    absolute_tolerance=1e-13,
    maximum_iterations=10_000,
)
plan = eqiora.resolve(
    model, mesh=mesh, spatial=eqiora.fem.MiniP1(), solve=linear, scaling=None,
)
result = eqiora.run(plan)
pressure = result.output(plan.capability.pressure)
pressure_values = pressure.values("vertex")
cylinder_force = result.boundary_force(geometry.selection("cylinder"))
inlet_flux = result.boundary_flux(geometry.selection("inlet"))
outlet_flux = result.boundary_flux(geometry.selection("outlet"))

print(result.plan_key)
print(result.solve)
print("pressure", min(pressure_values), max(pressure_values), "Pa")
print("cylinder force on fluid", cylinder_force.on_domain, "N/m")
print("net flux", inlet_flux.value + outlet_flux.value, "m^2/s")
```

This example shows the distinction Eqiora is built around: exact geometry and model meaning
remain immutable, meshing and solver choices live in explicit resolved plans,
and every returned output stays tied to the same Geometry, Model, Mesh, Plan,
and Result lineage.
[Walk through the pressure result](https://eqiora.org/gallery/exact-cylinder-steady-stokes/)
or run the complete
[`examples/python/exact_cylinder_stokes.py`](examples/python/exact_cylinder_stokes.py)
script with optional Matplotlib output.

For a bounded local file check, the installed `eqiora` binary accepts
`eqiora check <MODEL_PATH>`. It reads one UTF-8 regular file, prints only a
structural comparison fingerprint when the current Model is accepted, and
prints bounded normalized diagnostics when compilation rejects it.

Local agents can separately compile/check one in-memory Eqiora source through
the `eqiora-mcp` subprocess. It exposes exactly one bounded MCP `2026-07-28`
tool over newline-delimited stdio and returns either structured compiler
diagnostics or the current Model descriptor and comparison fingerprint. Python
is the primary execution API; Studio consumes the same Rust-owned model semantics.

## One model, two layers

Eqiora treats block diagrams, state charts, PDEs, and acausal physical
networks as views of the same small semantic kernel. A canonical model is a
network of typed relations, activations, and signal or conserving
connections. Numerical choices—mesh, discretization, solver, schedule, CPU,
GPU, or distributed execution—are typed policies resolved into an immutable
**Plan**.

That separation is enforced by one traceable path:

```text
.eqi → compile(geometry) → resolve(typed policies) → Plan → Run / Result
```

Source, Python, Studio, and future visual editors therefore create
transactions against one Rust-owned model semantics; none is a second
authority.

## Current alpha

The release includes bounded, reproducible vertical slices for the semantic
kernel and language, reference hybrid execution, scalar Operator IR,
one-to-three-dimensional scalar elliptic FEM/FVM paths, selected host/CUDA/MPI
adapters, implicit differentiation, versioned artifacts, Python model
construction and execution, and a thin Studio projection. The exact domain,
platform, method, and maturity of each slice are recorded in the
[capability matrix](docs/capability-matrix.md); the
[architecture guide](docs/architecture.md) explains their boundaries.

Pre-1.0 authoring APIs may change as the project converges on its final public
surface. Eqiora is not certified for safety-critical or production engineering
decisions.

## Project

- [Website and documentation](https://eqiora.org)
- [Python package](https://pypi.org/project/eqiora/)
- [Capabilities](docs/capability-matrix.md)
- [Published benchmarks](docs/benchmarks.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Governance](GOVERNANCE.md)
- [Release policy](docs/development/python-release-policy.md)

Eqiora is developed in public under the
[Apache License 2.0](LICENSE). Contributions require a
[Developer Certificate of Origin](CONTRIBUTING.md#developer-certificate-of-origin)
sign-off.
