# Examples

Use examples to learn the shape of the public API. Use registered evidence to
decide whether a capability exists for your exact problem.

## Orientation

Start with the semantic Rust quickstart, then use the installed Python spatial workflow:

```bash
cargo run --locked -p eqiora --example quickstart
python examples/python/exact_cylinder_stokes.py
```

The
[quickstart example](https://github.com/nkiyohara/eqiora/blob/main/crates/eqiora/examples/quickstart.rs)
compiles and runs a scalar decay model — the smallest complete path through the
facade. The
[exact-cylinder example](https://github.com/nkiyohara/eqiora/blob/main/examples/python/exact_cylinder_stokes.py)
is the spatial counterpart. The
[example index](https://github.com/nkiyohara/eqiora/blob/main/examples/README.md)
keeps orientation paths intentionally small.

## A spatial problem, end to end

The equation excerpt below comes from the verification-only
[`org.example.poisson`](https://github.com/nkiyohara/eqiora/tree/main/packages/org.example.poisson)
package; the public lifecycle walkthrough then uses the installed exact-cylinder workflow.

### The problem

On the unit square, with the value pinned to zero on all four sides:

```text
-div(grad(u)) = 2 pi^2 sin(pi x) sin(pi y)
```

That source was chosen so the exact solution is known:
`u(x, y) = sin(pi x) sin(pi y)`. Knowing the answer in advance is what turns a
run into a measurement.

In the Eqiora language the whole problem is a set of declarations:

```text
model Main {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  // ... three more boundaries ...
  representation scalar_space = continuum;

  field potential on square as scalar_space: 1 = 0;
  parameter wave_number: 1 / m = 3.141592653589793;
  parameter source_scale: 1 / m ^ 2 = 19.739208802178716;

  relation balance continuous on square {
    -div(grad(potential))
      - source_scale
        * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) = 0; }
  // ... one boundary relation per side ...
}
```

Read that as a statement of *what must be true*. It names a domain, a field, two
dimensioned parameters, and five relations. It does not name a mesh, an element,
a quadrature rule, a linear solver, a tolerance, or a machine — because none of
those are properties of the problem.

### Stage 1 — Geometry, Mesh, and Model

Python owns the concrete geometry once, realizes a typed mesh, and supplies the
same Geometry to the equations-only component:

```python
graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(fluid, named_topology={...})
mesh_plan = eqiora.meshing.resolve(geometry, eqiora.meshing.GmshMesher(...))
mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
model = eqiora.compile(path=source_path, geometry=geometry, parameters={...})
```

Compilation owns mathematical meaning; the meshing provider owns realization
of the exact caller-authored source. The `.eqi` file does not repeat the box or
circle.

### Stage 2 — Plan

```python
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.MiniP1(),
    solve=eqiora.solve.Linear(
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    ),
    scaling=None,
)
```

The root resolver admits typed numerical policies against the actual Model.
Physics comes from the Model; a policy does not select an application facade.

### Stage 3 — Run and observe

```python
result = eqiora.run(plan)
pressure = result.output(plan.pressure_field)
evidence = eqiora.fluid.steady_stokes_evidence(result)
```

The common Result retains the exact Plan binding and exposes complete outputs
only through Model-bound field handles. Typed observations remain separate from
the transport object. The complete runnable path is
[`examples/python/exact_cylinder_stokes.py`](../../examples/python/exact_cylinder_stokes.py).

## Numerical and physical slices

The repository's verified cases include bounded paths for:

- Cartesian and simplicial finite-element and finite-volume problems;
- time integration, DAE initialization, events, and reset;
- packages and conserving physical networks;
- fluid, solid, and fixed-reference or moving-mesh FSI;
- CPU, MPI, and CUDA execution contracts;
- Python modeling, arrays, differentiation, and framework adapters.

Those bullets are navigation categories, not blanket support claims. Choose a
case from the [evidence catalog](evidence/index.md), then read its manifest and
README for its exact claim and nonclaims.

The Poisson example above is orientation, not evidence. It establishes nothing
about accuracy, performance, or support for any other problem. The registered
counterparts with claims and falsifiers are
[`numerics.compiled-cartesian-poisson-q1-2d`](https://github.com/nkiyohara/eqiora/blob/main/verify/numerics/compiled-cartesian-poisson-q1-2d/README.md)
and
[`packages.typed-execution-lineage`](https://github.com/nkiyohara/eqiora/blob/main/verify/packages/typed-execution-lineage/README.md).

## Close a new capability

Contributors should follow the repository's
[high-risk and parallel development guide](https://github.com/nkiyohara/eqiora/blob/main/docs/development/vertical-slice-development.md):
a narrow claim, its existing invariant owner, an ordinary execution path, the
evidence needed for the changed claim, and a truthful capability-matrix update.
Ordinary work does not create a new contract artifact, independent oracle, or
lane. High-risk scientific, public, persisted, security, or trust changes keep
their specific independent evidence and review.
