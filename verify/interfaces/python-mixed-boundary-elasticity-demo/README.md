# Installed Python mixed-boundary elasticity root Plan

This case verifies the installed Python model-first structural path:

```python
geometry = eqiora.GeometryGraph(2)
body = geometry.rectangle("body", origin=(0.0, 0.0), size=(1.0, 1.0))
geometry = geometry.build(body)

model = eqiora.compile(
    path=component_source,
    geometry=geometry,
    parameters={...},
)
mesh = eqiora.mesh(
    geometry,
    eqiora.MeshRequest(provider=eqiora.CartesianMesher(cells=(16, 16))),
)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.Q1(),
    solver=eqiora.solve.Linear(...),
    scaling=None,
)
result = eqiora.run(plan)
displacement = plan.field("displacement")
output = result.output(displacement)
observation = eqiora.solid.linear_elasticity_evidence(result)
figure = eqiora.matplotlib.plot_deformed_field(
    result,
    field=displacement,
    scale=1.0,
)
```

The `.eqi` file owns equations, fields, parameters, and abstract supports.
Python owns the sole concrete rectangle and mesh. Physics comes from the Model;
the root resolver receives typed spatial and solver policies. The common Result
is read only with the exact Plan-owned field selector, while the native run
output preserves the exact Plan pairing used to construct the structural
observation.

Scientific values and tolerances remain unchanged and are owned by
[`solid.mixed-boundary-elasticity-2d`](../../solid/mixed-boundary-elasticity-2d/README.md).
This case checks the installed composition, common displacement output,
reaction/body-force/bounds relationships, foreign-field and cross-physics
rejection, fail-closed policy selection, and the headless caller-owned plot.
It also checks physical absence of the displaced `LinearElasticity` intent,
specialized Plan and resolver, and `plot_displacement` shim.

It does not claim stress, strain or traction recovery, convergence, other mesh
families, 3D, nonlinear or dynamic structure, FSI, interactive viewing, exact
pixels, or scientific validation from rendering. The executable boundary is
recorded in [`case.toml`](case.toml).
